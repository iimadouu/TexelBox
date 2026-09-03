use std::collections::HashMap;
use std::sync::Mutex;
use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;
const EMBEDDED: &[(&str, &str)] = &[
    ("en-US", include_str!("../../../locales/en-US.ftl")),
    ("es-ES", include_str!("../../../locales/es-ES.ftl")),
];
struct Inner {
    bundles: HashMap<String, FluentBundle<FluentResource>>,
    current: String,
    fallback: String,
}
pub struct LocaleManager {
    inner: Mutex<Inner>,
}
impl LocaleManager {
    pub fn new() -> Self {
        let mut bundles = HashMap::new();
        for (tag, src) in EMBEDDED {
            let lang: LanguageIdentifier = tag.parse().expect("valid language tag");
            let resource = FluentResource::try_new(src.to_string())
                .unwrap_or_else(|(_, errs)| panic!("broken FTL in {tag}: {errs:?}"));
            let mut bundle = FluentBundle::new_concurrent(vec![lang]);
            bundle.add_resource(resource).expect("duplicate FTL key");
            bundles.insert(tag.to_string(), bundle);
        }
        Self {
            inner: Mutex::new(Inner {
                bundles,
                current: "en-US".to_string(),
                fallback: "en-US".to_string(),
            }),
        }
    }
    pub fn available() -> Vec<&'static str> {
        EMBEDDED.iter().map(|(tag, _)| *tag).collect()
    }
    pub fn set_language(&self, tag: &str) -> bool {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.bundles.contains_key(tag) {
            inner.current = tag.to_string();
            true
        } else {
            false
        }
    }
    pub fn language(&self) -> String {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).current.clone()
    }
    pub fn tr(&self, key: &str) -> String {
        self.tr_args(key, &[])
    }
    pub fn tr_args(&self, key: &str, args: &[(&str, FluentValue<'_>)]) -> String {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let order = [inner.current.as_str(), inner.fallback.as_str()];
        let mut errors = vec![];
        for tag in order {
            let Some(bundle) = inner.bundles.get(tag) else { continue };
            let Some(msg) = bundle.get_message(key) else { continue };
            let pattern = msg.value().expect("message has value");
            let fa = if args.is_empty() { None } else {
                let mut fa = FluentArgs::new();
                for (k, v) in args {
                    fa.set(k.to_string(), v.clone());
                }
                Some(fa)
            };
            let out = bundle.format_pattern(pattern, fa.as_ref(), &mut errors);
            if errors.is_empty() {
                return out.into_owned();
            }
            errors.clear();
        }
        key.to_string()
    }
}
impl Default for LocaleManager {
    fn default() -> Self {
        Self::new()
    }
}
pub fn format_date(date: chrono::NaiveDate, locale_tag: &str) -> String {
    if locale_tag.starts_with("es") {
        date.format("%d/%m/%Y").to_string()
    } else {
        date.format("%b %e, %Y").to_string()
    }
}
pub fn num(v: f64) -> FluentValue<'static> {
    FluentValue::from(v)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    fn entries(src: &str) -> Vec<(String, bool)> {
        let resource = FluentResource::try_new(src.to_string()).expect("valid FTL");
        resource
            .entries()
            .filter_map(|e| match e {
                fluent_syntax::ast::Entry::Message(m) => {
                    Some((m.id.name.to_string(), m.value.is_some()))
                }
                _ => None,
            })
            .collect()
    }
    #[test]
    fn locale_key_parity() {
        let en: BTreeSet<String> = entries(EMBEDDED[0].1).into_iter().map(|(k, _)| k).collect();
        let es: BTreeSet<String> = entries(EMBEDDED[1].1).into_iter().map(|(k, _)| k).collect();
        let only_en: Vec<_> = en.difference(&es).collect();
        let only_es: Vec<_> = es.difference(&en).collect();
        assert!(only_en.is_empty(), "keys missing in es-ES: {only_en:?}");
        assert!(only_es.is_empty(), "keys missing in en-US: {only_es:?}");
        assert!(en.len() >= 100, "suspiciously few keys: {}", en.len());
    }
    #[test]
    fn no_valueless_messages() {
        for (tag, src) in EMBEDDED {
            for (key, has_value) in entries(src) {
                assert!(has_value, "{tag}: message '{key}' has no value");
            }
        }
    }
    #[test]
    fn runtime_switch_and_translate() {
        let lm = LocaleManager::new();
        assert_eq!(lm.language(), "en-US");
        let en = lm.tr("settings-title");
        assert!(lm.set_language("es-ES"));
        assert!(!lm.set_language("fr-FR"));
        let es = lm.tr("settings-title");
        assert_ne!(en, es, "settings-title should differ between locales");
        assert_eq!(lm.language(), "es-ES");
        assert_eq!(lm.tr("definitely-not-a-key"), "definitely-not-a-key");
    }
    #[test]
    fn number_and_date_formatting() {
        let d = chrono::NaiveDate::from_ymd_opt(2026, 8, 28).unwrap();
        assert_eq!(format_date(d, "es-ES"), "28/08/2026");
        assert_eq!(format_date(d, "en-US"), "Aug 28, 2026");
        assert!(matches!(num(1234.5), FluentValue::Number(_)));
    }
}
