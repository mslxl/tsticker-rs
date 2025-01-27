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

republish tag:
    git tag --delete {{ tag }}
    git push origin --delete {{ tag }}
    git tag {{ tag }}
    git push origin --tags

publish tag:
    git tag {{ tag}}
    git push origin --tags