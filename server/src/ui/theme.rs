//! Theme management and CSS definitions

use dioxus::prelude::*;

/// Theme mode
#[derive(Clone, Copy, PartialEq)]
pub enum ThemeMode {
    Light,
    Dark,
}

impl ThemeMode {
    pub fn toggle(&self) -> Self {
        match self {
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::Light,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
        }
    }
}

/// Global CSS with theme variables
pub const GLOBAL_CSS: &str = r#"
/* CSS Reset and Base Styles */
* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

:root[data-theme="light"] {
    --bg-primary: #ffffff;
    --bg-secondary: #f5f7fa;
    --bg-tertiary: #e5e9f0;
    --text-primary: #2e3440;
    --text-secondary: #4c566a;
    --text-muted: #6c7a89;
    --accent-primary: #5e81ac;
    --accent-success: #a3be8c;
    --accent-warning: #ebcb8b;
    --accent-error: #bf616a;
    --accent-info: #88c0d0;
    --border-color: #d8dee9;
    --shadow: rgba(0, 0, 0, 0.1);
}

:root[data-theme="dark"] {
    --bg-primary: #2e3440;
    --bg-secondary: #3b4252;
    --bg-tertiary: #434c5e;
    --text-primary: #eceff4;
    --text-secondary: #d8dee9;
    --text-muted: #a8b0c0;
    --accent-primary: #81a1c1;
    --accent-success: #a3be8c;
    --accent-warning: #ebcb8b;
    --accent-error: #bf616a;
    --accent-info: #88c0d0;
    --border-color: #4c566a;
    --shadow: rgba(0, 0, 0, 0.3);
}

body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    font-size: 16px;
    line-height: 1.5;
    background-color: var(--bg-primary);
    color: var(--text-primary);
    transition: background-color 0.2s ease, color 0.2s ease;
}

/* Layout */
.app-container {
    display: flex;
    flex-direction: column;
    min-height: 100vh;
}

.header {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    height: 60px;
    background-color: var(--bg-secondary);
    border-bottom: 1px solid var(--border-color);
    display: flex;
    align-items: center;
    padding: 0 24px;
    gap: 24px;
    z-index: 100;
}

.main-layout {
    display: flex;
    margin-top: 60px;
    min-height: calc(100vh - 60px);
}

.sidebar {
    position: fixed;
    left: 0;
    top: 60px;
    width: 240px;
    height: calc(100vh - 60px);
    background-color: var(--bg-secondary);
    border-right: 1px solid var(--border-color);
    padding: 24px 0;
    overflow-y: auto;
}

.main-content {
    margin-left: 240px;
    flex: 1;
    padding: 32px;
    background-color: var(--bg-primary);
}

/* Typography */
h1 {
    font-size: 2rem;
    font-weight: 600;
    margin-bottom: 16px;
    color: var(--text-primary);
}

h2 {
    font-size: 1.5rem;
    font-weight: 600;
    margin-bottom: 12px;
    color: var(--text-primary);
}

h3 {
    font-size: 1.25rem;
    font-weight: 600;
    margin-bottom: 8px;
    color: var(--text-primary);
}

/* Buttons */
.btn {
    padding: 8px 16px;
    border-radius: 8px;
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
    border: none;
    outline: none;
}

.btn-primary {
    background-color: var(--accent-primary);
    color: white;
}

.btn-primary:hover {
    opacity: 0.9;
    transform: translateY(-1px);
}

.btn-secondary {
    background-color: var(--bg-tertiary);
    color: var(--text-primary);
}

.btn-secondary:hover {
    background-color: var(--border-color);
}

/* Cards */
.card {
    background-color: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 12px;
    padding: 24px;
    box-shadow: 0 2px 8px var(--shadow);
}

/* Navigation */
.nav-link {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 24px;
    color: var(--text-secondary);
    text-decoration: none;
    transition: all 0.2s ease;
    font-weight: 500;
}

.nav-link:hover {
    background-color: var(--bg-tertiary);
    color: var(--text-primary);
}

.nav-link.active {
    background-color: var(--accent-primary);
    color: white;
}

/* Badge */
.badge {
    display: inline-block;
    padding: 4px 12px;
    border-radius: 9999px;
    font-size: 0.75rem;
    font-weight: 600;
}

.badge-success {
    background-color: var(--accent-success);
    color: white;
}

.badge-warning {
    background-color: var(--accent-warning);
    color: var(--text-primary);
}

.badge-error {
    background-color: var(--accent-error);
    color: white;
}

.badge-info {
    background-color: var(--accent-info);
    color: white;
}

/* Responsive */
@media (max-width: 1024px) {
    .sidebar {
        transform: translateX(-100%);
        transition: transform 0.3s ease;
    }

    .sidebar.open {
        transform: translateX(0);
    }

    .main-content {
        margin-left: 0;
    }
}
"#;

/// Inject global CSS into the document
pub fn inject_global_css() -> Element {
    rsx! {
        style { dangerous_inner_html: GLOBAL_CSS }
    }
}
