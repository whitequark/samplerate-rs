# samplerate-rs

Bindings for the [libsamplerate][] (aka Secret Rabbit Code) audio sample rate converter.

[libsamplerate]: http://www.mega-nerd.com/libsamplerate/

## Dependencies

Add this to your `Cargo.toml`:

```toml
[dependencies]
samplerate = "0.1"
```

To link to the system libsamplerate instead of the vendored one, use the `samplerate-sys/system` feature:

```toml
[dependencies]
samplerate-sys = { version = "0.1", features = ["system"] }
```

These bindings do not depend on `std`, and libsamplerate does not depend on anything but the C standard library.

## Usage

See documentation.

## License

[2-clause BSD](LICENSE.txt)
