//! The `conformance_report/1` report: per-point results, the gate and its renderings.

use std::fmt::Write as _;

use serde_json::{Value, json};

/// Schema name of the JSON report.
pub const SCHEMA: &str = "conformance_report/1";
/// A point that uses more than this share of its tolerance gets a warning (ADR-31).
pub const WARN_TOL_USED_PCT: f64 = 60.0;

/// How a point came out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Within tolerance, cross-check within its band.
    Pass,
    /// Outside tolerance, or the cross-check disagrees.
    Fail,
    /// Needs a capability that does not exist yet (`requires`).
    Deferred,
    /// The procedure could not produce a valid measurement.
    NoConvergence,
    /// The procedure ran out of simulated time.
    Timeout,
}

impl Status {
    /// The name used in the report.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Deferred => "deferred",
            Self::NoConvergence => "no_convergence",
            Self::Timeout => "timeout",
        }
    }
}

/// An independent static estimate of the same quantity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrossCheck {
    /// The static value.
    pub value: f64,
    /// Measured minus static, percent of static.
    pub delta_pct: f64,
    /// Largest allowed `|delta_pct|`.
    pub band_pct: f64,
}

impl CrossCheck {
    /// A cross-check of `measured` against `value` with a band.
    #[must_use]
    pub fn new(measured: f64, value: f64, band_pct: f64) -> Self {
        Self {
            value,
            delta_pct: (measured - value) / value * 100.0,
            band_pct,
        }
    }

    /// Inside the band.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.delta_pct.abs() <= self.band_pct
    }
}

/// A further cross-check, named: the ground rolls have two (ADR-32).
#[derive(Clone, Debug, PartialEq)]
pub struct NamedCheck {
    /// What it checks.
    pub name: String,
    /// The comparison.
    pub check: CrossCheck,
}

/// One envelope point's result.
#[derive(Clone, Debug, PartialEq)]
pub struct PointResult {
    /// Point id from the aircraft's envelope.
    pub id: String,
    /// Metric name.
    pub metric: String,
    /// Authored target.
    pub target: f64,
    /// Measured value (NaN when there is none).
    pub measured: f64,
    /// Tolerance, percent.
    pub tol_pct: f64,
    /// Outcome.
    pub status: Status,
    /// Procedure used.
    pub method: String,
    /// Static cross-check, if the procedure has one.
    pub cross_check: Option<CrossCheck>,
    /// Further cross-checks; every one must agree.
    pub extra_checks: Vec<NamedCheck>,
    /// Simulated time the procedure flew, s.
    pub sim_seconds: f64,
    /// Procedure-specific measurement quality figures.
    pub settle_stats: Value,
    /// Anything worth reading.
    pub notes: Vec<String>,
}

impl PointResult {
    /// Measured minus target, percent of target.
    #[must_use]
    pub fn error_pct(&self) -> f64 {
        (self.measured - self.target) / self.target * 100.0
    }

    /// Share of the tolerance used, percent.
    #[must_use]
    pub fn tol_used_pct(&self) -> f64 {
        self.error_pct().abs() / self.tol_pct * 100.0
    }

    /// Settles the status from the measurement: pass if within tolerance and the cross-check
    /// agrees. Statuses already set to something other than pass are kept.
    pub fn grade(&mut self) {
        if self.status != Status::Pass {
            return;
        }
        if !self.measured.is_finite() {
            self.status = Status::NoConvergence;
            return;
        }
        let within = self.error_pct().abs() <= self.tol_pct;
        let agrees = self.cross_check.is_none_or(|c| c.ok());
        if !agrees {
            self.notes
                .push("static cross-check outside its band".into());
        }
        let mut agrees = agrees;
        for e in &self.extra_checks {
            if !e.check.ok() {
                self.notes
                    .push(format!("cross-check `{}` outside its band", e.name));
                agrees = false;
            }
        }
        self.status = if within && agrees {
            Status::Pass
        } else {
            Status::Fail
        };
    }

    /// The target as authored: content stores `f32`, so print its shortest `f32` form.
    fn authored(v: f64) -> f64 {
        (v as f32).to_string().parse().unwrap_or(v)
    }

    fn json(&self) -> Value {
        let num = |v: f64| if v.is_finite() { json!(v) } else { Value::Null };
        json!({
            "id": self.id,
            "metric": self.metric,
            "target": num(Self::authored(self.target)),
            "measured": num(self.measured),
            "error_pct": num(self.error_pct()),
            "tol_pct": num(Self::authored(self.tol_pct)),
            "tol_used_pct": num(self.tol_used_pct()),
            "status": self.status.name(),
            "method": self.method,
            "cross_check": self.cross_check.map(|c| json!({
                "value": num(c.value),
                "delta_pct": num(c.delta_pct),
                "band_pct": num(c.band_pct),
                "ok": c.ok(),
            })),
            "extra_checks": self.extra_checks.iter().map(|e| json!({
                "name": e.name,
                "value": num(e.check.value),
                "delta_pct": num(e.check.delta_pct),
                "band_pct": num(e.check.band_pct),
                "ok": e.check.ok(),
            })).collect::<Vec<_>>(),
            "sim_seconds": num(self.sim_seconds),
            "settle_stats": self.settle_stats,
            "notes": self.notes,
        })
    }
}

/// The whole run.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    /// Aircraft id.
    pub aircraft: String,
    /// Content hash of the pack, hex.
    pub content_hash: String,
    /// Build that produced the report.
    pub build: String,
    /// Results in envelope order.
    pub points: Vec<PointResult>,
}

impl Report {
    /// Why the gate fails, empty when it passes: any point neither passed nor deferred, and a
    /// deferred set different from `expected_deferred` (ADR-31: exactly the ground rolls).
    #[must_use]
    pub fn gate_failures(&self, expected_deferred: &[String]) -> Vec<String> {
        let mut out = Vec::new();
        for p in &self.points {
            if !matches!(p.status, Status::Pass | Status::Deferred) {
                out.push(format!("{}: {}", p.id, p.status.name()));
            }
        }
        let mut deferred: Vec<&str> = self
            .points
            .iter()
            .filter(|p| p.status == Status::Deferred)
            .map(|p| p.id.as_str())
            .collect();
        deferred.sort_unstable();
        let mut expected: Vec<&str> = expected_deferred.iter().map(String::as_str).collect();
        expected.sort_unstable();
        if deferred != expected {
            out.push(format!(
                "deferred points are {deferred:?}, expected exactly {expected:?}"
            ));
        }
        out
    }

    /// Points passing but using more than [`WARN_TOL_USED_PCT`] of their tolerance.
    #[must_use]
    pub fn warnings(&self) -> Vec<String> {
        self.points
            .iter()
            .filter(|p| p.status == Status::Pass && p.tol_used_pct() > WARN_TOL_USED_PCT)
            .map(|p| format!("{}: uses {:.0} % of its tolerance", p.id, p.tol_used_pct()))
            .collect()
    }

    /// The `conformance_report/1` document.
    #[must_use]
    pub fn to_json(&self, expected_deferred: &[String]) -> Value {
        let failures = self.gate_failures(expected_deferred);
        json!({
            "schema": SCHEMA,
            "aircraft": self.aircraft,
            "content_hash": self.content_hash,
            "build": self.build,
            "points": self.points.iter().map(PointResult::json).collect::<Vec<_>>(),
            "gate": {
                "pass": failures.is_empty(),
                "failures": failures,
                "warnings": self.warnings(),
                "expected_deferred": expected_deferred,
            },
        })
    }

    /// A Markdown table for people.
    #[must_use]
    pub fn to_markdown(&self, expected_deferred: &[String]) -> String {
        let mut s = format!(
            "## Performance conformance: {}\n\ncontent {}\n\n\
             | Point | Metric | Target | Measured | Error | Tol | Used | Cross-check | Status |\n\
             |---|---|---|---|---|---|---|---|---|\n",
            self.aircraft,
            &self.content_hash[..self.content_hash.len().min(16)]
        );
        for p in &self.points {
            let (measured, err, used) = if p.measured.is_finite() {
                (
                    format!("{:.4}", p.measured),
                    format!("{:+.2} %", p.error_pct()),
                    format!("{:.0} %", p.tol_used_pct()),
                )
            } else {
                ("-".into(), "-".into(), "-".into())
            };
            let mut cross = p.cross_check.map_or("-".into(), |c| {
                format!(
                    "{:.4} ({:+.2} %{})",
                    c.value,
                    c.delta_pct,
                    if c.ok() { "" } else { " OUT" }
                )
            });
            for e in &p.extra_checks {
                let _ = write!(
                    cross,
                    "; {} {:+.2} %{}",
                    e.name,
                    e.check.delta_pct,
                    if e.check.ok() { "" } else { " OUT" }
                );
            }
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} | {} | {} % | {} | {} | {} |",
                p.id,
                p.metric,
                p.target as f32,
                measured,
                err,
                p.tol_pct,
                used,
                cross,
                p.status.name()
            );
        }
        let failures = self.gate_failures(expected_deferred);
        s += "\n";
        for w in self.warnings() {
            let _ = writeln!(s, "warning: {w}");
        }
        if failures.is_empty() {
            s += "gate: pass\n";
        } else {
            for f in failures {
                let _ = writeln!(s, "gate: FAIL: {f}");
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(id: &str, measured: f64, status: Status) -> PointResult {
        PointResult {
            id: id.into(),
            metric: "max_mach".into(),
            target: 1.22,
            measured,
            tol_pct: 3.0,
            status,
            method: "test".into(),
            cross_check: Some(CrossCheck::new(measured, 1.23, 0.3)),
            extra_checks: Vec::new(),
            sim_seconds: 1.0,
            settle_stats: json!({}),
            notes: Vec::new(),
        }
    }

    #[test]
    fn grading_needs_the_tolerance_and_the_cross_check() {
        let mut p = point("a", 1.23, Status::Pass);
        p.grade();
        assert_eq!(p.status, Status::Pass);
        assert!((p.tol_used_pct() - 27.3).abs() < 0.1);
        let mut far = point("b", 1.30, Status::Pass);
        far.grade();
        assert_eq!(far.status, Status::Fail, "6.6 % off a 3 % tolerance");
        let mut disagree = point("c", 1.23, Status::Pass);
        disagree.cross_check = Some(CrossCheck::new(1.23, 1.25, 0.3));
        disagree.grade();
        assert_eq!(disagree.status, Status::Fail, "cross-check 1.6 % off");
        let mut none = point("d", f64::NAN, Status::Pass);
        none.grade();
        assert_eq!(none.status, Status::NoConvergence);
    }

    #[test]
    fn every_extra_check_must_agree_too() {
        let mut p = point("a", 1.23, Status::Pass);
        p.extra_checks.push(NamedCheck {
            name: "energy".into(),
            check: CrossCheck::new(1.0, 1.0001, 1.0),
        });
        p.grade();
        assert_eq!(p.status, Status::Pass);
        let mut bad = point("b", 1.23, Status::Pass);
        bad.extra_checks.push(NamedCheck {
            name: "energy".into(),
            check: CrossCheck::new(1.0, 1.05, 1.0),
        });
        bad.grade();
        assert_eq!(bad.status, Status::Fail);
        assert!(bad.notes.iter().any(|n| n.contains("`energy`")));
        let doc = Report {
            aircraft: "core:ac/test".into(),
            content_hash: "00".repeat(32),
            build: "test".into(),
            points: vec![bad],
        };
        let text = doc.to_markdown(&[]);
        assert!(text.contains("energy -4.76 % OUT"), "{text}");
        assert_eq!(
            doc.to_json(&[])["points"][0]["extra_checks"][0]["ok"],
            false
        );
    }

    #[test]
    fn the_gate_wants_exactly_the_expected_deferrals() {
        let report = Report {
            aircraft: "core:ac/test".into(),
            content_hash: "00".repeat(32),
            build: "test".into(),
            points: vec![
                point("a", 1.23, Status::Pass),
                point("roll", f64::NAN, Status::Deferred),
            ],
        };
        let expected = vec!["roll".to_owned()];
        assert_eq!(report.gate_failures(&expected), Vec::<String>::new());
        assert_eq!(report.gate_failures(&[]).len(), 1, "an unexpected deferral");
        let two = vec!["roll".to_owned(), "other".to_owned()];
        assert_eq!(report.gate_failures(&two).len(), 1, "a missing deferral");
        let doc = report.to_json(&expected);
        assert_eq!(doc["schema"], SCHEMA);
        assert_eq!(doc["gate"]["pass"], true);
        assert_eq!(doc["points"][1]["measured"], Value::Null);
        assert!(report.to_markdown(&expected).contains("gate: pass"));
    }
}
