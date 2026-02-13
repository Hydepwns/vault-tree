use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Create a test vault with a standard structure for testing.
pub fn create_test_vault() -> TempDir {
    let dir = TempDir::new().unwrap();

    write_note(
        dir.path(),
        "note1.md",
        "---\ntitle: Note 1\ntags: [rust]\ndate: 2025-01-18\n---\n\n# Hello World\n\nContent with [[note2]]\nThis is a test note.\n",
    );

    write_note(
        dir.path(),
        "note2.md",
        "---\ntitle: Note 2\ntags: [mcp]\n---\n\n# Another Note\n\nHello again, world!\n",
    );

    fs::create_dir(dir.path().join("subdir")).unwrap();
    write_note(
        dir.path().join("subdir"),
        "nested.md",
        "# Nested\n\nLinks to [[note1]]",
    );

    fs::create_dir(dir.path().join(".obsidian")).unwrap();
    fs::write(dir.path().join(".obsidian/config.json"), "{}").unwrap();

    dir
}

fn write_note(dir: impl AsRef<Path>, name: &str, content: &str) {
    fs::write(dir.as_ref().join(name), content).unwrap();
}
