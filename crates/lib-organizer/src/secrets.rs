use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretType {
    PrivateKey,
    SshKey,
    AgeKey,
    GpgKey,
    PasswordManager,
    EnvFile,
    Credentials,
    WalletSeed,
    RecoveryKit,
    ApiKey,
    AwsCredentials,
    Certificate,
}

impl SecretType {
    pub fn severity(&self) -> Severity {
        match self {
            Self::PrivateKey | Self::SshKey | Self::AgeKey | Self::GpgKey => Severity::Critical,
            Self::WalletSeed | Self::RecoveryKit | Self::PasswordManager => Severity::Critical,
            Self::AwsCredentials | Self::ApiKey | Self::Credentials => Severity::High,
            Self::EnvFile | Self::Certificate => Severity::Medium,
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::PrivateKey => "Private key file",
            Self::SshKey => "SSH private key",
            Self::AgeKey => "Age encryption key",
            Self::GpgKey => "GPG private key",
            Self::PasswordManager => "Password manager export/backup",
            Self::EnvFile => "Environment file with secrets",
            Self::Credentials => "Credentials file",
            Self::WalletSeed => "Cryptocurrency wallet/seed",
            Self::RecoveryKit => "Recovery kit or backup codes",
            Self::ApiKey => "API key or token",
            Self::AwsCredentials => "AWS credentials",
            Self::Certificate => "Certificate with private key",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Critical => write!(f, "CRITICAL"),
            Self::High => write!(f, "HIGH"),
            Self::Medium => write!(f, "MEDIUM"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SensitiveFile {
    pub path: PathBuf,
    pub secret_type: SecretType,
    pub reason: String,
    pub matched_by: MatchSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchSource {
    Filename,
    Extension,
    Content,
}

impl SensitiveFile {
    pub fn severity(&self) -> Severity {
        self.secret_type.severity()
    }
}

/// Filename patterns that indicate sensitive files
const SENSITIVE_FILENAME_PATTERNS: &[(&str, SecretType)] = &[
    // SSH keys
    ("id_rsa", SecretType::SshKey),
    ("id_dsa", SecretType::SshKey),
    ("id_ecdsa", SecretType::SshKey),
    ("id_ed25519", SecretType::SshKey),
    // Age keys
    ("age_key", SecretType::AgeKey),
    ("age-key", SecretType::AgeKey),
    // Password managers
    ("1password emergency kit", SecretType::PasswordManager),
    ("emergency kit", SecretType::PasswordManager),
    ("lastpass", SecretType::PasswordManager),
    ("bitwarden", SecretType::PasswordManager),
    ("keepass", SecretType::PasswordManager),
    ("dashlane", SecretType::PasswordManager),
    // Recovery kits
    ("recovery-kit", SecretType::RecoveryKit),
    ("recovery kit", SecretType::RecoveryKit),
    ("recoverykit", SecretType::RecoveryKit),
    ("backup codes", SecretType::RecoveryKit),
    ("backup-codes", SecretType::RecoveryKit),
    ("2fa backup", SecretType::RecoveryKit),
    ("mfa backup", SecretType::RecoveryKit),
    ("totp backup", SecretType::RecoveryKit),
    // Wallet/Crypto
    ("seed phrase", SecretType::WalletSeed),
    ("seedphrase", SecretType::WalletSeed),
    ("mnemonic", SecretType::WalletSeed),
    ("wallet backup", SecretType::WalletSeed),
    ("keystore", SecretType::WalletSeed),
    // Credentials
    ("credentials", SecretType::Credentials),
    ("secrets", SecretType::Credentials),
    ("password", SecretType::Credentials),
    // AWS
    ("aws_credentials", SecretType::AwsCredentials),
    ("aws-credentials", SecretType::AwsCredentials),
];

/// File extensions that indicate sensitive files
const SENSITIVE_EXTENSIONS: &[(&str, SecretType)] = &[
    ("pem", SecretType::PrivateKey),
    ("key", SecretType::PrivateKey),
    ("p12", SecretType::Certificate),
    ("pfx", SecretType::Certificate),
    ("gpg", SecretType::GpgKey),
    ("asc", SecretType::GpgKey),
    ("kdbx", SecretType::PasswordManager),
    ("kdb", SecretType::PasswordManager),
    ("1pux", SecretType::PasswordManager),
];

/// Exact filenames that are sensitive
const SENSITIVE_EXACT_NAMES: &[(&str, SecretType)] = &[
    (".env", SecretType::EnvFile),
    (".env.local", SecretType::EnvFile),
    (".env.production", SecretType::EnvFile),
    (".env.development", SecretType::EnvFile),
    ("credentials.json", SecretType::Credentials),
    ("service-account.json", SecretType::Credentials),
    ("gcloud-credentials.json", SecretType::Credentials),
];

/// Content patterns that indicate secrets (regex-like simple patterns)
const SENSITIVE_CONTENT_PATTERNS: &[(&str, SecretType)] = &[
    ("-----BEGIN RSA PRIVATE KEY-----", SecretType::PrivateKey),
    ("-----BEGIN PRIVATE KEY-----", SecretType::PrivateKey),
    ("-----BEGIN EC PRIVATE KEY-----", SecretType::PrivateKey),
    ("-----BEGIN OPENSSH PRIVATE KEY-----", SecretType::SshKey),
    ("-----BEGIN DSA PRIVATE KEY-----", SecretType::SshKey),
    ("-----BEGIN PGP PRIVATE KEY BLOCK-----", SecretType::GpgKey),
    ("AGE-SECRET-KEY-", SecretType::AgeKey),
    ("AKIA", SecretType::AwsCredentials), // AWS Access Key ID prefix
    ("aws_secret_access_key", SecretType::AwsCredentials),
    ("sk-", SecretType::ApiKey), // OpenAI, Stripe style
    ("sk_live_", SecretType::ApiKey),
    ("sk_test_", SecretType::ApiKey),
    ("ghp_", SecretType::ApiKey), // GitHub PAT
    ("gho_", SecretType::ApiKey), // GitHub OAuth
    ("github_pat_", SecretType::ApiKey),
    ("xox", SecretType::ApiKey), // Slack tokens
];

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub check_content: bool,
    pub max_file_size: u64,
    pub include_hidden: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            check_content: false,
            max_file_size: 1024 * 1024, // 1MB
            include_hidden: true,       // Secrets are often in hidden files
        }
    }
}

impl ScanOptions {
    pub fn default_with_content() -> Self {
        Self {
            check_content: true,
            ..Self::default()
        }
    }
}

/// Scan a directory for sensitive files
pub fn scan_for_secrets(path: &Path, options: &ScanOptions) -> Vec<SensitiveFile> {
    let should_include = |e: &walkdir::DirEntry| -> bool {
        let name = e.file_name().to_string_lossy();
        name != ".git"
            && (options.include_hidden
                || !name.starts_with('.')
                || name == "."
                || e.file_type().is_dir())
    };

    let check_file = |entry: walkdir::DirEntry| -> Option<SensitiveFile> {
        if !entry.file_type().is_file() {
            return None;
        }

        let file_path = entry.path();

        check_filename(file_path)
            .or_else(|| check_extension(file_path))
            .or_else(|| {
                options
                    .check_content
                    .then(|| entry.metadata().ok())
                    .flatten()
                    .filter(|m| m.len() <= options.max_file_size)
                    .and_then(|_| check_content(file_path))
            })
    };

    let mut results: Vec<_> = WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_entry(should_include)
        .filter_map(|e| e.ok())
        .filter_map(check_file)
        .collect();

    results.sort_by_key(|r| std::cmp::Reverse(r.severity()));
    results
}

/// Scan specific files for secrets
pub fn scan_files_for_secrets(files: &[PathBuf], options: &ScanOptions) -> Vec<SensitiveFile> {
    let check_file = |path: &PathBuf| -> Option<SensitiveFile> {
        if !path.is_file() {
            return None;
        }

        check_filename(path)
            .or_else(|| check_extension(path))
            .or_else(|| {
                options
                    .check_content
                    .then(|| fs::metadata(path).ok())
                    .flatten()
                    .filter(|m| m.len() <= options.max_file_size)
                    .and_then(|_| check_content(path))
            })
    };

    let mut results: Vec<_> = files.iter().filter_map(check_file).collect();
    results.sort_by_key(|r| std::cmp::Reverse(r.severity()));
    results
}

fn check_filename(path: &Path) -> Option<SensitiveFile> {
    let filename = path.file_name()?.to_string_lossy().to_lowercase();

    // Check exact matches first
    for (name, secret_type) in SENSITIVE_EXACT_NAMES {
        if filename == *name {
            return Some(SensitiveFile {
                path: path.to_path_buf(),
                secret_type: secret_type.clone(),
                reason: format!("Exact filename match: {}", name),
                matched_by: MatchSource::Filename,
            });
        }
    }

    // Check .env.* pattern
    if filename.starts_with(".env.") || filename.starts_with("env.") {
        return Some(SensitiveFile {
            path: path.to_path_buf(),
            secret_type: SecretType::EnvFile,
            reason: "Environment file pattern".to_string(),
            matched_by: MatchSource::Filename,
        });
    }

    // Check pattern matches
    for (pattern, secret_type) in SENSITIVE_FILENAME_PATTERNS {
        if filename.contains(pattern) {
            return Some(SensitiveFile {
                path: path.to_path_buf(),
                secret_type: secret_type.clone(),
                reason: format!("Filename contains: {}", pattern),
                matched_by: MatchSource::Filename,
            });
        }
    }

    None
}

fn check_extension(path: &Path) -> Option<SensitiveFile> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();

    for (sensitive_ext, secret_type) in SENSITIVE_EXTENSIONS {
        if ext == *sensitive_ext {
            return Some(SensitiveFile {
                path: path.to_path_buf(),
                secret_type: secret_type.clone(),
                reason: format!("Sensitive extension: .{}", sensitive_ext),
                matched_by: MatchSource::Extension,
            });
        }
    }

    None
}

fn check_content(path: &Path) -> Option<SensitiveFile> {
    let content = fs::read_to_string(path).ok()?;

    for (pattern, secret_type) in SENSITIVE_CONTENT_PATTERNS {
        if content.contains(pattern) {
            return Some(SensitiveFile {
                path: path.to_path_buf(),
                secret_type: secret_type.clone(),
                reason: format!("Content contains: {}", truncate_pattern(pattern)),
                matched_by: MatchSource::Content,
            });
        }
    }

    None
}

fn truncate_pattern(pattern: &str) -> String {
    if pattern.len() > 20 {
        format!("{}...", &pattern[..20])
    } else {
        pattern.to_string()
    }
}

/// Format scan results for display
pub fn format_results(results: &[SensitiveFile]) -> String {
    if results.is_empty() {
        return "No sensitive files detected.".to_string();
    }

    let mut output = format!("Found {} sensitive file(s):\n\n", results.len());

    for (i, file) in results.iter().enumerate() {
        output.push_str(&format!(
            "{}. [{}] {}\n   {}: {}\n   Reason: {}\n\n",
            i + 1,
            file.severity(),
            file.path.display(),
            file.secret_type.description(),
            match file.matched_by {
                MatchSource::Filename => "filename",
                MatchSource::Extension => "extension",
                MatchSource::Content => "content",
            },
            file.reason
        ));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn detects_ssh_key_by_name() {
        let dir = TempDir::new().unwrap();
        let key_path = dir.path().join("id_rsa");
        fs::write(&key_path, "fake key").unwrap();

        let results = scan_for_secrets(dir.path(), &ScanOptions::default());

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].secret_type, SecretType::SshKey);
    }

    #[test]
    fn detects_env_file() {
        let dir = TempDir::new().unwrap();
        let env_path = dir.path().join(".env");
        fs::write(&env_path, "SECRET=value").unwrap();

        let results = scan_for_secrets(dir.path(), &ScanOptions::default());

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].secret_type, SecretType::EnvFile);
    }

    #[test]
    fn detects_pem_by_extension() {
        let dir = TempDir::new().unwrap();
        let pem_path = dir.path().join("server.pem");
        fs::write(&pem_path, "certificate data").unwrap();

        let results = scan_for_secrets(dir.path(), &ScanOptions::default());

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].secret_type, SecretType::PrivateKey);
    }

    #[test]
    fn detects_private_key_by_content() {
        let dir = TempDir::new().unwrap();
        let key_path = dir.path().join("random.txt");
        fs::write(&key_path, "data\n-----BEGIN RSA PRIVATE KEY-----\nkey\n").unwrap();

        let options = ScanOptions::default_with_content();
        let results = scan_for_secrets(dir.path(), &options);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].secret_type, SecretType::PrivateKey);
        assert_eq!(results[0].matched_by, MatchSource::Content);
    }

    #[test]
    fn detects_1password_emergency_kit() {
        let dir = TempDir::new().unwrap();
        let kit_path = dir.path().join("1Password Emergency Kit A3-XXXX.pdf");
        fs::write(&kit_path, "pdf content").unwrap();

        let results = scan_for_secrets(dir.path(), &ScanOptions::default());

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].secret_type, SecretType::PasswordManager);
    }

    #[test]
    fn detects_age_key() {
        let dir = TempDir::new().unwrap();
        let key_path = dir.path().join("age_key.txt");
        fs::write(&key_path, "AGE-SECRET-KEY-ABC123").unwrap();

        let options = ScanOptions::default_with_content();
        let results = scan_for_secrets(dir.path(), &options);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].secret_type, SecretType::AgeKey);
    }

    #[test]
    fn detects_recovery_kit() {
        let dir = TempDir::new().unwrap();
        let kit_path = dir.path().join("proton-recovery-kit.pdf");
        fs::write(&kit_path, "pdf").unwrap();

        let results = scan_for_secrets(dir.path(), &ScanOptions::default());

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].secret_type, SecretType::RecoveryKit);
    }

    #[test]
    fn skips_git_directory() {
        let dir = TempDir::new().unwrap();
        let git_dir = dir.path().join(".git");
        fs::create_dir(&git_dir).unwrap();
        fs::write(git_dir.join("id_rsa"), "key").unwrap();

        let results = scan_for_secrets(dir.path(), &ScanOptions::default());

        assert!(results.is_empty());
    }

    #[test]
    fn sorts_by_severity() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".env"), "x").unwrap(); // Medium
        fs::write(dir.path().join("id_rsa"), "x").unwrap(); // Critical

        let results = scan_for_secrets(dir.path(), &ScanOptions::default());

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].severity(), Severity::Critical);
        assert_eq!(results[1].severity(), Severity::Medium);
    }

    #[test]
    fn scan_specific_files() {
        let dir = TempDir::new().unwrap();
        let key = dir.path().join("id_rsa");
        let normal = dir.path().join("readme.txt");
        fs::write(&key, "key").unwrap();
        fs::write(&normal, "readme").unwrap();

        let results = scan_files_for_secrets(&[key, normal], &ScanOptions::default());

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].secret_type, SecretType::SshKey);
    }
}
