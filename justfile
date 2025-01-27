build:
    cargo build --release

test:
    cargo run --bin tsticker-cli -- \
        https://t.me/addstickers/in_FIEBEC_by_NaiDrawBot \
        https://t.me/addstickers/in_BFEBEC_by_NaiDrawBot \
        https://t.me/addstickers/in_CIHHDC_by_NaiDrawBot \

test-release: build
    ./target/release/tsticker-cli \
        https://t.me/addstickers/in_FIEBEC_by_NaiDrawBot \
        https://t.me/addstickers/in_BFEBEC_by_NaiDrawBot \
        https://t.me/addstickers/in_CIHHDC_by_NaiDrawBot \
