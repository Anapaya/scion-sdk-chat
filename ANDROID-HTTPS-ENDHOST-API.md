# Android: an HTTPS endhost API fails with an uninitialized platform verifier

Repository to fix: <https://github.com/Anapaya/scion-sdk>

## Symptom

An Android application that builds a `ScionHttp3Client` against an **HTTPS** endhost API fails at
connect time with:

```
Expect rustls-platform-verifier to be initialized
```

The same application against an **HTTP** endhost API works. So does the identical configuration from
the Rust SDK on macOS. The failure is Android-only and scheme-only.

## Reproduce

1. Build a client whose endhost API is a production SNAP:

   ```kotlin
   ScionHttp3Client.Builder(context)
       .endhostApi("https://snap01.chwin1.in7.prod.fsnets.com:5001/")
       .authToken(snapToken)
       .trust(TrustAnchors.insecureNoVerify())   // any of the three, it makes no difference
       .build()
   ```

2. Send any request.

Against `http://10.0.2.2:8099` (a PocketSCION `chat-dev`) the same code connects and the application
runs normally. That is why this has gone unnoticed: every example and every local test uses a
PocketSCION endhost API, which is plain HTTP.

Versions: `scion-http3-android` 0.8.0, `rustls-platform-verifier` 0.7.0, Android API 36 emulator,
`minSdk` 24.

## What the failure is

The error is not about the SCION server's certificate. It is raised while verifying the **endhost
API's own TLS certificate**, by an HTTP client living inside `libscion_http3_ffi.so`.

Evidence:

- `strings libscion_http3_ffi.so` contains `Expect rustls-platform-verifier to be initialized`,
  `JavaVM singleton uninitialized`, and the paths
  `rustls-platform-verifier-0.7.0/src/verification/android.rs` and
  `rustls-platform-verifier-0.7.0/src/android.rs`.
- `rustls-platform-verifier` on Android needs the JavaVM and a `Context` registered before it can
  read the device trust store.
- The shared library defines **no `JNI_OnLoad`**, so nothing registers the JavaVM when it loads.
- The UniFFI surface exposes no initializer. The complete list of entry points is: `clone_cancelhandle`,
  `clone_scionhttp`, `constructor_cancelhandle_new`, `constructor_scionhttp`, `free_cancelhandle`,
  `free_scionhttp`, `func_default_client_config`, `func_internal_panic_for_test`,
  `method_cancelhandle_cancel`, `method_scionhttp`. An application therefore *cannot* initialize it,
  even knowing that it must.

## Why `TrustAnchors` does not help

`TrustAnchors` governs the QUIC connection to the SCION server. All three values behave identically
here, `insecureNoVerify()` included, because the endhost API is a separate HTTPS call that never
consults that setting. An application that deliberately disables verification still cannot reach an
HTTPS endhost API.

## The shape of a fix

The SDK already solves this exact problem once, on the other side.
`com/anapaya/scion/http3/internal/AndroidPlatform.kt` reads the platform's anchors through
`TrustManagerFactory`, encodes them as PEM, and passes them across the FFI:

```kotlin
override fun anchorsPem(): ByteArray {
    val factory = TrustManagerFactory.getInstance(TrustManagerFactory.getDefaultAlgorithm())
    factory.init(null as KeyStore?)
    // ... returns PemEncoder.encode(anchors)
}
```

Its own doc comment says this "applies the platform's rules, including the application's network
security configuration". That is exactly what the endhost API client needs, and it is already being
computed.

Two candidate fixes, in the order I would try them:

1. **Give the endhost API client the same PEM anchors.** Where the internal HTTP client is built
   (reqwest, judging by the symbols in the library), install those anchors as roots instead of
   relying on the platform verifier. No JNI, no new FFI surface, and it reuses machinery that
   already exists and is already tested on the SCION path.

2. **Initialize `rustls-platform-verifier` on Android.** Add an FFI entry point that takes the
   JavaVM and `Context` and calls the crate's Android init, then call it from
   `ScionHttp3Client.Builder`, which **already requires a `Context`** — its doc comment says the
   Context is needed for "reading the device's trust anchors, and watching for network changes".
   More faithful to the platform (revocation, per-application network security config), but it needs
   a new FFI function and care over JNI reference lifetimes.

Whichever is chosen, please also consider:

- **Fail with a usable message.** Today the error names an internal crate and gives the application
  no action. If initialization is genuinely required of the caller, say so and name the call.
- **Cover it in CI.** Every current test path uses a PocketSCION HTTP endhost API, which is why this
  is reachable at all. A test against any HTTPS endhost API on Android would have caught it.
## Why iOS is unaffected

Both platforms bundle the same crate. Only the backend differs:

| | backend compiled in | init strings in the binary |
| --- | --- | --- |
| `libscion_http3_ffi.so` (Android) | `verification/android.rs`, `android.rs` | `Expect rustls-platform-verifier to be initialized`, `JavaVM singleton uninitialized` |
| `libscion_http3_ffi.a` (iOS) | `verification/apple.rs` | none |

The Apple backend reaches Security.framework directly from Rust, so it has no initialization
contract. The Android backend has to call back into Java to read the system trust store, so it needs
the JavaVM and a `Context` registered first. Android is the only platform with that requirement, and
it is unmet.

This is worth weighing when choosing between the two fixes above: fix 2 is Android-only plumbing for
a problem the other platforms do not have, while fix 1 puts every platform on the same path.

## Why it matters

This blocks every Android application that talks to a production SNAP, because production endhost
APIs are HTTPS. Only PocketSCION-based development works today. A Kotlin application can currently
reach a real SCION deployment in no way at all.
