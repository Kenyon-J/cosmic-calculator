app_id := "com.cosmic.calculator"
yml_file := app_id + ".yml"

# Download the official Flatpak dependency generator
get-generator:
    curl -O https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py

# Convert Cargo.lock into the offline cargo-sources.json manifest
gen-sources: get-generator
    python3 flatpak-cargo-generator.py Cargo.lock -o cargo-sources.json

# Build the Flatpak locally and run it to test
test-flatpak: gen-sources
    flatpak-builder --user --install --force-clean build-dir {{yml_file}}
    flatpak run {{app_id}}

clean:
    rm -rf build-dir .flatpak-builder flatpak-cargo-generator.py cargo-sources.json
