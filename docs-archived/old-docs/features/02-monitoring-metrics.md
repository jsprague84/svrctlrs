# Monitoring & Metrics Features

**Category:** Monitoring & Metrics  
**Current State:** Basic task execution tracking, no historical metrics  
**Gap:** No time-series data, no anomaly detection, limited plugin coverage  
**Priority:** Tier 2-3 (High value, medium-high effort)

---

## Overview

Expand monitoring capabilities beyond basic health checks to include time-series metrics, anomaly detection, custom metrics, and comprehensive infrastructure monitoring.

**Value Proposition:**
- Proactive issue detection through anomaly detection
- Historical trend analysis for capacity planning
- Custom metrics for application-specific monitoring
- Comprehensive infrastructure visibility

---

## Features

### 1. Time-Series Metrics Database

**Priority:** Tier 2 | **Effort:** High | **Value:** High

Store and query historical metrics with retention policies and aggregation.

**Capabilities:**
- Store metrics from all plugins (CPU, memory, disk, network, etc.)
- Configurable retention policies (1hr@1min, 1day@5min, 1month@1hr, 1year@1day)
- Automatic downsampling and aggregation
- Export to Prometheus, InfluxDB, or Grafana
- Query API for custom time ranges

**Schema:**
```sql
CREATE TABLE metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    server_id INTEGER,
    plugin_id TEXT NOT NULL,
    metric_name TEXT NOT NULL,
    metric_value REAL NOT NULL,
    timestamp DATETIME NOT NULL,
    tags TEXT,  -- JSON: {"container": "nginx", "type": "cpu"}
    FOREIGN KEY (server_id) REFERENCES servers(id)
);

CREATE INDEX idx_metrics_lookup ON metrics(plugin_id, metric_name, timestamp);
CREATE INDEX idx_metrics_server ON metrics(server_id, timestamp);
```

**Retention Policy:**
```rust
struct RetentionPolicy {
    raw: Duration::hours(1),      // Keep 1 hour at full resolution
    aggregated_5m: Duration::days(1),   // 1 day at 5min intervals
    aggregated_1h: Duration::days(30),  // 30 days at 1hr intervals
    aggregated_1d: Duration::days(365), // 1 year at 1day intervals
}
```

---

### 2. Anomaly Detection

**Priority:** Tier 3 | **Effort:** High | **Value:** High

ML-based anomaly detection with automatic baseline learning.

**Capabilities:**
- Learn normal behavior patterns over time
- Detect deviations from baseline
- Automatic threshold adjustment
- Predictive alerts ("disk will be full in 3 days")
- Correlation analysis (CPU spike + memory leak detection)

**Algorithms:**
- Moving average with standard deviation
- Exponential smoothing
- Seasonal decomposition
- Isolation forest for outlier detection

**Implementation:**
```rust
struct AnomalyDetector {
    baseline: Baseline,
    sensitivity: f64,
    learning_window: Duration,
}

impl AnomalyDetector {
    async fn detect(&self, metric: &Metric) -> Option<Anomaly> {
        let zscore = (metric.value - self.baseline.mean) / self.baseline.stddev;
        if zscore.abs() > self.sensitivity {
            Some(Anomaly {
                metric: metric.clone(),
                severity: self.calculate_severity(zscore),
                prediction: self.predict_future_state(metric),
            })
        } else {
            None
        }
    }
}
```

---

### 3. Custom Metrics Plugin

**Priority:** Tier 2 | **Effort:** Medium | **Value:** Medium

User-defined metrics via scripts and log parsing.

**Capabilities:**
- Execute custom scripts to collect metrics
- Parse log files for patterns
- Regex-based metric extraction
- JSON/XML response parsing
- HTTP endpoint polling

**Configuration:**
```yaml
custom_metrics:
  - name: "nginx_requests"
    type: "log_parse"
    file: "/var/log/nginx/access.log"
    pattern: '(\d+\.\d+\.\d+\.\d+).*"(GET|POST)'
    interval: "1m"
  
  - name: "app_response_time"
    type: "http"
    url: "http://localhost:3000/metrics"
    json_path: "$.response_time"
    interval: "30s"
  
  - name: "queue_depth"
    type: "script"
    command: "redis-cli LLEN myqueue"
    interval: "10s"
```

---

### 4. Network Monitoring Plugin

**Priority:** Tier 1 | **Effort:** Low | **Value:** High

Monitor network connectivity, ports, and SSL certificates.

**Capabilities:**
- Ping monitoring for uptime tracking
- Port availability checks (TCP/UDP)
- SSL certificate expiration monitoring
- DNS resolution monitoring
- Response time tracking
- Bandwidth usage monitoring

**Checks:**
```rust
pub struct NetworkMonitor {
    ping_targets: Vec<PingTarget>,
    port_checks: Vec<PortCheck>,
    ssl_checks: Vec<SslCheck>,
    dns_checks: Vec<DnsCheck>,
}

struct PingTarget {
    host: String,
    interval: Duration,
    timeout: Duration,
    alert_on_failure: bool,
}

struct SslCheck {
    domain: String,
    port: u16,
    warn_days: u32,  // Warn when < X days until expiration
}
```

**Notifications:**
- Alert when ping fails (host down)
- Alert when port closed (service down)
- Alert on SSL expiration (30/14/7 days)
- Alert on DNS resolution failure

---

### 5. Database Monitoring Plugin

**Priority:** Tier 2 | **Effort:** Medium | **Value:** High

Monitor database health, performance, and replication.

**Supported Databases:**
- PostgreSQL
- MySQL/MariaDB
- MongoDB
- Redis
- SQLite

**Capabilities:**
- Connection pool monitoring
- Query performance tracking
- Slow query detection
- Replication lag monitoring
- Backup verification
- Table/index size tracking

**Metrics Collected:**
```rust
struct DatabaseMetrics {
    connection_count: u32,
    active_queries: u32,
    slow_queries: u32,
    cache_hit_ratio: f64,
    replication_lag_seconds: Option<f64>,
    disk_usage_bytes: u64,
    uptime_seconds: u64,
}
```

---

### 6. Application Performance Monitoring (APM)

**Priority:** Tier 2 | **Effort:** Medium | **Value:** Medium

Monitor application-level metrics and performance.

**Capabilities:**
- HTTP response time tracking
- Error rate monitoring
- Request rate (throughput)
- Custom application metrics
- Distributed tracing (OpenTelemetry)

**Integration:**
- Parse application logs
- Poll metrics endpoints (/metrics, /health)
- Integrate with APM tools (Datadog, New Relic)

---

### 7. Log Analysis Plugin

**Priority:** Tier 2 | **Effort:** Medium | **Value:** Medium

Analyze logs for patterns, errors, and trends.

**Capabilities:**
- Pattern detection in logs
- Error frequency tracking
- Log-based alerting
- Anomaly detection in log volume
- Keyword search across servers

**Use Cases:**
- Alert on error rate spike
- Detect security events (failed logins)
- Track application errors
- Monitor audit logs

---

### 8. Bandwidth Monitoring

**Priority:** Tier 2 | **Effort:** Medium | **Value:** Low

Track network bandwidth usage and trends.

**Capabilities:**
- Interface bandwidth monitoring
- Traffic analysis by protocol
- Top talkers identification
- Bandwidth usage trends
- Alert on bandwidth thresholds

---

### 9. Service Dependency Mapping

**Priority:** Tier 3 | **Effort:** High | **Value:** Medium

Visualize service relationships and dependencies.

**Capabilities:**
- Automatic dependency discovery
- Service relationship visualization
- Impact analysis for outages
- Dependency health checks
- Critical path identification

**Visualization:**
```
nginx (healthy)
  ├─> app-server (healthy)
  │   ├─> database (healthy)
  │   └─> redis (unhealthy) ⚠️
  └─> static-files (healthy)
```

---

## Implementation Priority

**Phase 1 (v1.1.0):** Network monitoring, Certificate monitoring  
**Phase 2 (v1.2.0):** Time-series database, Database monitoring  
**Phase 3 (v1.3.0):** Custom metrics, Log analysis  
**Phase 4 (v2.0.0):** Anomaly detection, APM, Service mapping

---

## Technical Stack

**Time-Series Storage:**
- Option 1: SQLite with partitioning (simple, integrated)
- Option 2: InfluxDB (powerful, separate service)
- Option 3: TimescaleDB (PostgreSQL extension)

**Anomaly Detection:**
- Rust ML libraries: `smartcore`, `linfa`
- Statistical methods: moving average, z-score
- External: Prometheus Alertmanager

**Visualization:**
- Chart.js for frontend charts
- Grafana for advanced dashboards (optional)

---

## References

- [Prometheus Data Model](https://prometheus.io/docs/concepts/data_model/)
- [InfluxDB Schema Design](https://docs.influxdata.com/influxdb/v2.0/reference/key-concepts/)
- [OpenTelemetry](https://opentelemetry.io/)

---

**Last Updated:** 2025-11-25  
**Status:** Planning

