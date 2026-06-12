use crate::config::{AuthMethod, Config};
use crate::output::Output;
use anyhow::Result;
use rand::Rng;
use serde::Serialize;
use std::fmt::Write;
use std::fs;
use std::path::PathBuf;

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const MAGENTA: &str = "\x1b[35m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const BANNER_SUBTITLE_WIDTH: usize = 51;

const SUBTITLES: &[&str] = &[
    "apps :: incidents :: logs :: auth",
    "trace the weirdness :: ship the fix",
    "tail logs :: inspect apps :: close incidents",
    "from deploy panic to root cause",
    "observability without leaving your shell",
    "signals in :: answers out",
];

#[derive(Serialize)]
struct AboutResponse {
    version: String,
    platform: String,
    endpoint: String,
    default_org: Option<String>,
    auth: String,
    subtitle: String,
    suggested_commands: Vec<&'static str>,
}

pub fn show(format: Output) -> Result<()> {
    let config = Config::load()?;
    let subtitle = select_subtitle();
    let version = env!("CARGO_PKG_VERSION").to_string();
    let platform = format!("{} / {}", std::env::consts::OS, std::env::consts::ARCH);
    let endpoint = config
        .endpoint
        .as_deref()
        .unwrap_or("https://appsignal.com")
        .to_string();
    let default_org = config.org.clone().filter(|org| !org.is_empty());
    let auth = auth_summary(&config);
    let suggested_commands = vec![
        "appsignal-cli auth login",
        "appsignal-cli apps orgs",
        "appsignal-cli incidents list --app \"MyApp\" --environment production",
        "appsignal-cli logs tail --app \"MyApp\" --environment production",
    ];

    let response = AboutResponse {
        version: version.clone(),
        platform: platform.clone(),
        endpoint: endpoint.clone(),
        default_org,
        auth: auth.clone(),
        subtitle: subtitle.to_string(),
        suggested_commands: suggested_commands.clone(),
    };

    crate::output::print_with(response, format, move |w| {
        write!(
            w,
            "{}",
            render_about(
                &version,
                &platform,
                &endpoint,
                config.org.as_deref().unwrap_or("not set"),
                auth.as_str(),
                subtitle,
                &suggested_commands,
                colors_enabled(),
            )
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn render_about(
    version: &str,
    platform: &str,
    endpoint: &str,
    default_org: &str,
    auth: &str,
    subtitle: &str,
    suggested_commands: &[&str],
    color: bool,
) -> String {
    let mut output = String::new();
    for line in build_banner(subtitle) {
        writeln!(&mut output, "{}", paint_banner_line(&line, MAGENTA, color)).unwrap();
    }

    writeln!(&mut output).unwrap();

    write_kv(&mut output, "Version", version, MAGENTA, color);
    write_kv(&mut output, "Platform", platform, MAGENTA, color);
    write_kv(&mut output, "Endpoint", endpoint, MAGENTA, color);
    write_kv(&mut output, "Default org", default_org, MAGENTA, color);
    write_kv(&mut output, "Auth", auth, MAGENTA, color);

    writeln!(&mut output).unwrap();
    writeln!(&mut output, "{}", paint("Try these next:", BOLD, color)).unwrap();

    for command in suggested_commands {
        writeln!(
            &mut output,
            "  {} {}",
            paint(">", GREEN, color),
            paint(command, YELLOW, color)
        )
        .unwrap();
    }

    writeln!(&mut output).unwrap();
    writeln!(
        &mut output,
        "{}",
        paint(
            "Tip: `appsignal-cli --help` shows the full command tree.",
            DIM,
            color
        )
    )
    .unwrap();

    output
}

fn auth_summary(config: &Config) -> String {
    match config.auth_method() {
        Ok(AuthMethod::OAuth { expires_at, .. }) => {
            if config.oauth_token_expired() {
                "OAuth (expired, refresh on next API call)".to_string()
            } else if let Some(timestamp) = expires_at {
                let expiry = chrono::DateTime::from_timestamp(timestamp, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                    .unwrap_or_else(|| "unknown expiry".to_string());
                format!("OAuth (expires {})", expiry)
            } else {
                "OAuth".to_string()
            }
        }
        Err(_) => "Not authenticated".to_string(),
    }
}

fn build_banner(subtitle: &str) -> [String; 7] {
    [
        "+--------------------------------------------------------------+".to_string(),
        "|    .-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-.    |".to_string(),
        "|    |                 APPSIGNAL CLI                     |    |".to_string(),
        format!(
            "|    |{:^width$}|    |",
            truncate_for_banner(subtitle),
            width = BANNER_SUBTITLE_WIDTH
        ),
        "|    '-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-'    |".to_string(),
        "|           observability, from the terminal                  |".to_string(),
        "+--------------------------------------------------------------+".to_string(),
    ]
}

fn truncate_for_banner(text: &str) -> String {
    let count = text.chars().count();
    if count <= BANNER_SUBTITLE_WIDTH {
        return text.to_string();
    }

    let truncated: String = text.chars().take(BANNER_SUBTITLE_WIDTH - 3).collect();
    format!("{truncated}...")
}

fn write_kv(output: &mut String, label: &str, value: &str, accent: &str, color: bool) {
    let _ = writeln!(
        output,
        "  {} {} {}",
        paint("::", accent, color),
        paint(&format!("{label:10}"), BOLD, color),
        value
    );
}

fn paint(text: &str, code: &str, enabled: bool) -> String {
    if enabled {
        format!("{code}{text}{RESET}")
    } else {
        text.to_string()
    }
}

fn paint_banner_line(line: &str, accent: &str, enabled: bool) -> String {
    if !enabled {
        return line.to_string();
    }

    if line.len() >= 2 {
        let border = &line[..1];
        let tail = &line[line.len() - 1..];
        let inner = &line[1..line.len() - 1];
        return format!("{accent}{border}{RESET}{BOLD}{inner}{RESET}{accent}{tail}{RESET}");
    }

    line.to_string()
}

fn select_subtitle() -> &'static str {
    let previous = load_previous_subtitle_index();
    let index = next_subtitle_index(previous, SUBTITLES.len());
    save_previous_subtitle_index(index);
    SUBTITLES[index]
}

fn next_subtitle_index(previous: Option<usize>, len: usize) -> usize {
    if len <= 1 {
        return 0;
    }

    match previous.filter(|index| *index < len) {
        Some(previous) => (previous + rand::thread_rng().gen_range(1..len)) % len,
        None => rand::thread_rng().gen_range(0..len),
    }
}

fn load_previous_subtitle_index() -> Option<usize> {
    let path = subtitle_state_path()?;
    let raw = fs::read_to_string(path).ok()?;
    raw.trim().parse().ok()
}

fn save_previous_subtitle_index(index: usize) {
    let Some(path) = subtitle_state_path() else {
        return;
    };

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let _ = fs::write(path, index.to_string());
}

fn subtitle_state_path() -> Option<PathBuf> {
    let cache_dir = dirs::cache_dir()?;
    Some(cache_dir.join("appsignal").join("about_subtitle_index"))
}

fn colors_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OAuthCredentials;

    #[test]
    fn render_about_without_color_contains_core_sections() {
        let config = Config {
            org: Some("my-org".to_string()),
            oauth: Some(OAuthCredentials {
                access_token: "access-token".to_string(),
                refresh_token: Some("refresh-token".to_string()),
                expires_at: Some(1_900_000_000),
            }),
            ..Config::default()
        };

        let output = render_about(
            "1.2.3",
            "linux / x86_64",
            "https://appsignal.com",
            config.org.as_deref().unwrap_or("not set"),
            &auth_summary(&config),
            SUBTITLES[0],
            &[
                "appsignal-cli auth login",
                "appsignal-cli apps orgs",
                "appsignal-cli incidents list --app \"MyApp\" --environment production",
                "appsignal-cli logs tail --app \"MyApp\" --environment production",
            ],
            false,
        );

        assert!(output.contains("APPSIGNAL CLI"));
        assert!(output.contains(SUBTITLES[0]));
        assert!(output.contains(":: Version    1.2.3"));
        assert!(output.contains(":: Platform   linux / x86_64"));
        assert!(output.contains(":: Default org my-org"));
        assert!(output.contains(":: Auth       OAuth"));
        assert!(output.contains("appsignal-cli auth login"));
        assert!(!output.contains("\x1b["));
    }

    #[test]
    fn select_subtitle_returns_known_variant() {
        let subtitle = select_subtitle();
        assert!(SUBTITLES.contains(&subtitle));
    }

    #[test]
    fn next_subtitle_index_avoids_previous_when_possible() {
        for _ in 0..32 {
            assert_ne!(next_subtitle_index(Some(1), 3), 1);
        }
    }

    #[test]
    fn next_subtitle_index_handles_single_option() {
        assert_eq!(next_subtitle_index(Some(0), 1), 0);
    }

    #[test]
    fn truncate_for_banner_shortens_long_lines() {
        let text = "this subtitle is definitely much longer than forty eight characters";
        let truncated = truncate_for_banner(text);

        assert_eq!(truncated.chars().count(), BANNER_SUBTITLE_WIDTH);
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn auth_summary_reports_expired_oauth() {
        let config = Config {
            oauth: Some(OAuthCredentials {
                access_token: "access-token".to_string(),
                refresh_token: Some("refresh-token".to_string()),
                expires_at: Some(1),
            }),
            ..Config::default()
        };

        assert!(auth_summary(&config).contains("expired"));
    }
}
