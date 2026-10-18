//! A reader for the `Host` blocks of an OpenSSH `ssh_config`, enough to turn a pasted file
//! into servers: alias, `HostName`, `Port`, `User` and the first hop of `ProxyJump`. Wildcard
//! and negated patterns, `Match` blocks and every other keyword are ignored; the import tells
//! the user what it skipped.

/// One concrete `Host` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostEntry {
    /// The alias after `Host`, which becomes the server's name
    pub alias: String,
    /// `HostName`, else the alias
    pub hostname: String,
    pub port: i32,
    pub user: Option<String>,
    /// First hop of `ProxyJump`, as an alias or address (without `user@` and `:port`)
    pub proxy_jump: Option<String>,
}

/// What the parser found, and what it left out.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    pub hosts: Vec<HostEntry>,
    /// Patterns and blocks that cannot become a server, with the reason
    pub skipped: Vec<(String, String)>,
}

fn is_pattern(alias: &str) -> bool {
    alias.contains(['*', '?']) || alias.starts_with('!')
}

/// `ProxyJump user@host:port[,next…]` → `host` of the first hop.
fn first_jump(value: &str) -> Option<String> {
    let first = value.split(',').next()?.trim();
    if first.is_empty() || first.eq_ignore_ascii_case("none") {
        return None;
    }
    let host = first.rsplit('@').next().unwrap_or(first);
    // `[v6::addr]:port` or `host:port`
    let host = if let Some(rest) = host.strip_prefix('[') {
        rest.split(']').next().unwrap_or(rest)
    } else {
        host.split(':').next().unwrap_or(host)
    };
    (!host.is_empty()).then(|| host.to_string())
}

pub fn parse(text: &str) -> Parsed {
    let mut parsed = Parsed::default();
    // The aliases of the current block, None while inside `Match` or before the first `Host`
    let mut current: Option<Vec<HostEntry>> = None;

    let flush = |current: &mut Option<Vec<HostEntry>>, parsed: &mut Parsed| {
        if let Some(entries) = current.take() {
            parsed.hosts.extend(entries);
        }
    };

    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let (key, value) = match line.split_once(['=', ' ', '\t']) {
            Some((k, v)) => (k.trim(), v.trim().trim_start_matches('=').trim()),
            None => (line, ""),
        };
        let value = value.trim_matches('"');
        match key.to_ascii_lowercase().as_str() {
            "host" => {
                flush(&mut current, &mut parsed);
                let mut entries = Vec::new();
                for alias in value.split_whitespace() {
                    if is_pattern(alias) {
                        parsed
                            .skipped
                            .push((alias.to_string(), "a pattern, not a host".to_string()));
                    } else {
                        entries.push(HostEntry {
                            alias: alias.to_string(),
                            hostname: alias.to_string(),
                            port: 22,
                            user: None,
                            proxy_jump: None,
                        });
                    }
                }
                current = Some(entries);
            }
            "match" => {
                flush(&mut current, &mut parsed);
                parsed.skipped.push((
                    format!("Match {value}"),
                    "Match blocks are not imported".into(),
                ));
                current = None;
            }
            "hostname" => {
                if let Some(entries) = current.as_mut() {
                    for e in entries.iter_mut() {
                        // `%h` stands for the alias itself
                        e.hostname = value.replace("%h", &e.alias);
                    }
                }
            }
            "port" => {
                if let (Some(entries), Ok(port)) = (current.as_mut(), value.parse::<i32>()) {
                    if (1..=65535).contains(&port) {
                        for e in entries.iter_mut() {
                            e.port = port;
                        }
                    }
                }
            }
            "user" => {
                if let Some(entries) = current.as_mut() {
                    for e in entries.iter_mut() {
                        e.user = Some(value.to_string());
                    }
                }
            }
            "proxyjump" => {
                if let Some(entries) = current.as_mut() {
                    let hop = first_jump(value);
                    for e in entries.iter_mut() {
                        e.proxy_jump = hop.clone();
                    }
                }
            }
            _ => {}
        }
    }
    flush(&mut current, &mut parsed);
    parsed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_hosts_with_defaults_and_overrides() {
        let p = parse(
            "# my boxes\n\
             Host web\n  HostName web.example.net\n  User deploy\n  Port 2222\n\n\
             Host db   # trailing comment\n\
             Host bare\n",
        );
        assert_eq!(p.hosts.len(), 3);
        assert_eq!(
            p.hosts[0],
            HostEntry {
                alias: "web".into(),
                hostname: "web.example.net".into(),
                port: 2222,
                user: Some("deploy".into()),
                proxy_jump: None,
            }
        );
        assert_eq!(p.hosts[1].hostname, "db");
        assert_eq!(p.hosts[1].port, 22);
        assert_eq!(p.hosts[2].alias, "bare");
        assert!(p.skipped.is_empty());
    }

    #[test]
    fn skips_patterns_and_match_blocks_but_keeps_the_rest() {
        let p = parse(
            "Host *\n  ServerAliveInterval 30\n\
             Host *.lab !lab-old\n\
             Match host web\n  User x\n\
             Host good\n  User ops\n",
        );
        let names: Vec<_> = p.hosts.iter().map(|h| h.alias.as_str()).collect();
        assert_eq!(names, ["good"]);
        assert_eq!(p.hosts[0].user.as_deref(), Some("ops"));
        let skipped: Vec<_> = p.skipped.iter().map(|(s, _)| s.as_str()).collect();
        assert_eq!(skipped, ["*", "*.lab", "!lab-old", "Match host web"]);
    }

    #[test]
    fn proxy_jump_keeps_the_first_hop_host_only() {
        assert_eq!(first_jump("bastion"), Some("bastion".into()));
        assert_eq!(first_jump("ops@bastion:2200,other"), Some("bastion".into()));
        assert_eq!(first_jump("[2001:db8::1]:22"), Some("2001:db8::1".into()));
        assert_eq!(first_jump("none"), None);
        let p = parse("Host inner\n  ProxyJump admin@bastion:2200\n  HostName 10.0.0.5\n");
        assert_eq!(p.hosts[0].proxy_jump.as_deref(), Some("bastion"));
    }

    #[test]
    fn accepts_equals_syntax_quotes_and_multiple_aliases() {
        let p = parse("Host a b\n  HostName=\"%h.example\"\n  Port=22\n  User = me\n");
        assert_eq!(p.hosts.len(), 2);
        assert_eq!(p.hosts[0].hostname, "a.example");
        assert_eq!(p.hosts[1].hostname, "b.example");
        assert_eq!(p.hosts[1].user.as_deref(), Some("me"));
    }
}
