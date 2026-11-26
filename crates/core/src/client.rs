//! Clients: who the user's time goes to, and what each one paid.
//!
//! A client is recognised from the activity itself, without timers: a word
//! in the window title (the project folder shown by the terminal or the
//! editor, the company name, a chat name), a website, or an app. Payments
//! are entered by the user (or by their AI assistant), so zman can tell the
//! real rate per hour of every client.

use std::collections::BTreeMap;

use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};

use crate::category::domain_matches;
use crate::event::Event;
use crate::period::{self, Range};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Client {
    pub name: String,
    /// The user's own product or project: its hours count, but it is not
    /// someone who pays, so it has no rate per hour.
    pub own: bool,
    /// Words looked for in window titles, ignoring case, e.g. the project
    /// folder (`amcabi`) or the company name.
    pub keywords: Vec<String>,
    /// Websites of the client, subdomains included (`primepagos.com`).
    pub domains: Vec<String>,
    /// Apps used only for this client (`dualrec.exe`).
    pub apps: Vec<String>,
    pub payments: Vec<Payment>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Payment {
    /// Day the money came in or was billed, `YYYY-MM-DD`.
    pub date: String,
    pub amount: f64,
    /// `ARS`, `USD`...
    pub currency: String,
    pub note: String,
}

impl Client {
    pub fn matches(&self, event: &Event) -> bool {
        if let Some(domain) = event.domain() {
            if self.domains.iter().any(|d| !d.trim().is_empty() && domain_matches(d, &domain)) {
                return true;
            }
        }
        if self.apps.iter().any(|a| !a.trim().is_empty() && a.trim().eq_ignore_ascii_case(&event.app)) {
            return true;
        }
        let title = event.title.to_lowercase();
        let url = event.url().unwrap_or("").to_lowercase();
        self.keywords.iter().map(|k| k.trim().to_lowercase()).filter(|k| !k.is_empty()).any(|k| title.contains(&k) || url.contains(&k))
    }
}

/// The first client whose rules match `event`.
pub fn client_of<'a>(clients: &'a [Client], event: &Event) -> Option<&'a Client> {
    clients.iter().find(|c| c.matches(event))
}

/// An amount of money in one currency.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Money {
    pub currency: String,
    pub amount: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClientUsage {
    pub name: String,
    pub own: bool,
    pub ms: i64,
    /// Paid within the range, per currency.
    pub paid: Vec<Money>,
    /// Paid divided by hours, per currency. Empty for own projects and when
    /// nothing was tracked or paid.
    pub per_hour: Vec<Money>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClientReport {
    pub total_ms: i64,
    /// Time that matched no client.
    pub unassigned_ms: i64,
    /// Longest first; clients with neither time nor payments are left out.
    pub clients: Vec<ClientUsage>,
}

/// Hours and payments per client within `range`. Payment dates are read in `tz`.
pub fn report(events: &[Event], range: Range, clients: &[Client], tz: &TimeZone) -> ClientReport {
    let mut ms_by: Vec<i64> = vec![0; clients.len()];
    let mut total_ms = 0;
    let mut unassigned_ms = 0;
    for event in events {
        let ms = (event.end_ms().min(range.end_ms) - event.start_ms.max(range.start_ms)).max(0);
        if ms == 0 {
            continue;
        }
        total_ms += ms;
        match clients.iter().position(|c| c.matches(event)) {
            Some(i) => ms_by[i] += ms,
            None => unassigned_ms += ms,
        }
    }

    let mut list: Vec<ClientUsage> = clients
        .iter()
        .zip(ms_by)
        .map(|(client, ms)| {
            let mut paid: BTreeMap<String, f64> = BTreeMap::new();
            for p in &client.payments {
                let Ok(date) = period::parse_date(&p.date) else { continue };
                let Ok(at) = period::start_of_day(date, tz) else { continue };
                if at >= range.start_ms && at < range.end_ms {
                    let currency = if p.currency.trim().is_empty() { "ARS".to_string() } else { p.currency.trim().to_uppercase() };
                    *paid.entry(currency).or_default() += p.amount;
                }
            }
            let hours = ms as f64 / 3_600_000.0;
            let per_hour = if client.own || hours < 0.01 {
                Vec::new()
            } else {
                paid.iter().map(|(c, a)| Money { currency: c.clone(), amount: (a / hours * 100.0).round() / 100.0 }).collect()
            };
            ClientUsage {
                name: client.name.clone(),
                own: client.own,
                ms,
                paid: paid.into_iter().map(|(currency, amount)| Money { currency, amount }).collect(),
                per_hour,
            }
        })
        .filter(|c| c.ms > 0 || !c.paid.is_empty())
        .collect();
    list.sort_by(|a, b| b.ms.cmp(&a.ms).then_with(|| a.name.cmp(&b.name)));
    ClientReport { total_ms, unassigned_ms, clients: list }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const MIN: i64 = 60_000;
    /// 2026-11-20T00:00:00Z.
    const DAY: i64 = 1_795_132_800_000;

    fn event(app: &str, title: &str, url: &str, start_min: i64, minutes: i64) -> Event {
        Event {
            id: format!("{app}-{start_min}"),
            device_id: "d".into(),
            start_ms: DAY + start_min * MIN,
            duration_ms: minutes * MIN,
            app: app.into(),
            title: title.into(),
            data: if url.is_empty() { json!({}) } else { json!({ "url": url }) },
            updated_at_ms: 0,
        }
    }

    fn clients() -> Vec<Client> {
        vec![
            Client {
                name: "AMCABI".into(),
                keywords: vec!["amcabi".into()],
                payments: vec![
                    Payment { date: "2026-11-20".into(), amount: 300_000.0, currency: "ars".into(), note: String::new() },
                    Payment { date: "2026-10-01".into(), amount: 1.0, currency: "ARS".into(), note: String::new() },
                ],
                ..Client::default()
            },
            Client { name: "PrimePagos".into(), domains: vec!["primepagos.com".into()], ..Client::default() },
            Client { name: "Dualrec".into(), own: true, apps: vec!["dualrec.exe".into()], keywords: vec!["dualrec".into()], ..Client::default() },
        ]
    }

    #[test]
    fn recognises_clients_by_keyword_site_and_app() {
        let c = clients();
        assert_eq!(client_of(&c, &event("WindowsTerminal.exe", "URIELZALCMAN: amcabi_web", "", 0, 1)).unwrap().name, "AMCABI");
        assert_eq!(client_of(&c, &event("chrome.exe", "Cobranzas", "https://cobranzas.primepagos.com/x", 0, 1)).unwrap().name, "PrimePagos");
        assert_eq!(client_of(&c, &event("Dualrec.exe", "Recording", "", 0, 1)).unwrap().name, "Dualrec");
        assert!(client_of(&c, &event("chrome.exe", "WhatsApp", "https://web.whatsapp.com", 0, 1)).is_none());
    }

    #[test]
    fn report_counts_hours_payments_and_rate() {
        let events = vec![
            event("WindowsTerminal.exe", "URIELZALCMAN: amcabi", "", 9 * 60, 90),
            event("WindowsTerminal.exe", "URIELZALCMAN: amcabi", "", 12 * 60, 30),
            event("Dualrec.exe", "Recording", "", 14 * 60, 60),
            event("chrome.exe", "Funny cats", "https://youtube.com", 15 * 60, 20),
        ];
        let range = Range { start_ms: DAY, end_ms: DAY + 1440 * MIN };
        let r = report(&events, range, &clients(), &TimeZone::UTC);
        assert_eq!(r.total_ms, 200 * MIN);
        assert_eq!(r.unassigned_ms, 20 * MIN);
        assert_eq!(r.clients.len(), 2);
        let amcabi = &r.clients[0];
        assert_eq!((amcabi.name.as_str(), amcabi.ms), ("AMCABI", 120 * MIN));
        assert_eq!(amcabi.paid, vec![Money { currency: "ARS".into(), amount: 300_000.0 }]);
        assert_eq!(amcabi.per_hour, vec![Money { currency: "ARS".into(), amount: 150_000.0 }]);
        let dualrec = &r.clients[1];
        assert!(dualrec.own && dualrec.per_hour.is_empty());
    }
}
