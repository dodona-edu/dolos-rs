pub mod fingerprints;
pub mod hashes;
pub mod region;
pub mod tokenizer;

/// Read a JSON fixture.
#[cfg(test)]
fn read_fixture<T: serde::de::DeserializeOwned>(path: impl AsRef<std::path::Path>) -> T {
    let file = std::fs::File::open(path).unwrap();
    serde_json::from_reader(std::io::BufReader::new(file)).unwrap()
}
