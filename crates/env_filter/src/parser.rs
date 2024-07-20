use log::LevelFilter;

use crate::Directive;
use crate::FilterOp;

#[derive(Default, Debug)]
pub(crate) struct ParseResult {
    pub(crate) directives: Vec<Directive>,
    pub(crate) filter: Option<FilterOp>,
    pub(crate) errors: Vec<String>,
}

impl ParseResult {
    fn add_directive(&mut self, directive: Directive) {
        self.directives.push(directive);
    }

    fn set_filter(&mut self, filter: FilterOp) {
        self.filter = Some(filter);
    }

    fn add_error(&mut self, message: String) {
        self.errors.push(message);
    }
}

/// Parse a logging specification string (e.g: `crate1,crate2::mod3,crate3::x=error/foo`)
/// and return a vector with log directives.
pub(crate) fn parse_spec(spec: &str) -> ParseResult {
    let mut result = ParseResult::default();

    let mut parts = spec.split('/');
    let mods = parts.next();
    let filter = parts.next();
    if parts.next().is_some() {
        result.add_error(format!("invalid logging spec '{}' (too many '/'s)", spec));
        return result;
    }
    if let Some(m) = mods {
        for s in m.split(',').map(|ss| ss.trim()) {
            if s.is_empty() {
                continue;
            }
            let mut parts = s.split('=');
            let (log_level, name) =
                match (parts.next(), parts.next().map(|s| s.trim()), parts.next()) {
                    (Some(part0), None, None) => {
                        // if the single argument is a log-level string or number,
                        // treat that as a global fallback
                        match part0.parse() {
                            Ok(num) => (num, None),
                            Err(_) => (LevelFilter::max(), Some(part0)),
                        }
                    }
                    (Some(part0), Some(""), None) => (LevelFilter::max(), Some(part0)),
                    (Some(part0), Some(part1), None) => {
                        if let Ok(num) = part1.parse() {
                            (num, Some(part0))
                        } else {
                            result.add_error(format!("invalid logging spec '{}'", part1));
                            continue;
                        }
                    }
                    _ => {
                        result.add_error(format!("invalid logging spec '{}'", s));
                        continue;
                    }
                };

            result.add_directive(Directive {
                name: name.map(|s| s.to_owned()),
                level: log_level,
            });
        }
    }

    if let Some(filter) = filter {
        match FilterOp::new(filter) {
            Ok(filter_op) => result.set_filter(filter_op),
            Err(err) => result.add_error(format!("invalid regex filter - {}", err)),
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use log::LevelFilter;

    use super::{parse_spec, ParseResult};

    #[test]
    fn parse_spec_valid() {
        let ParseResult {
            directives: dirs,
            filter,
            ..
        } = parse_spec("crate1::mod1=error,crate1::mod2,crate2=debug");

        assert_eq!(dirs.len(), 3);
        assert_eq!(dirs[0].name, Some("crate1::mod1".to_owned()));
        assert_eq!(dirs[0].level, LevelFilter::Error);

        assert_eq!(dirs[1].name, Some("crate1::mod2".to_owned()));
        assert_eq!(dirs[1].level, LevelFilter::max());

        assert_eq!(dirs[2].name, Some("crate2".to_owned()));
        assert_eq!(dirs[2].level, LevelFilter::Debug);
        assert!(filter.is_none());
    }

    #[test]
    fn parse_spec_invalid_crate() {
        // test parse_spec with multiple = in specification
        let ParseResult {
            directives: dirs,
            filter,
            ..
        } = parse_spec("crate1::mod1=warn=info,crate2=debug");

        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].name, Some("crate2".to_owned()));
        assert_eq!(dirs[0].level, LevelFilter::Debug);
        assert!(filter.is_none());
    }

    #[test]
    fn parse_spec_invalid_level() {
        // test parse_spec with 'noNumber' as log level
        let ParseResult {
            directives: dirs,
            filter,
            ..
        } = parse_spec("crate1::mod1=noNumber,crate2=debug");

        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].name, Some("crate2".to_owned()));
        assert_eq!(dirs[0].level, LevelFilter::Debug);
        assert!(filter.is_none());
    }

    #[test]
    fn parse_spec_string_level() {
        // test parse_spec with 'warn' as log level
        let ParseResult {
            directives: dirs,
            filter,
            ..
        } = parse_spec("crate1::mod1=wrong,crate2=warn");

        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].name, Some("crate2".to_owned()));
        assert_eq!(dirs[0].level, LevelFilter::Warn);
        assert!(filter.is_none());
    }

    #[test]
    fn parse_spec_empty_level() {
        // test parse_spec with '' as log level
        let ParseResult {
            directives: dirs,
            filter,
            ..
        } = parse_spec("crate1::mod1=wrong,crate2=");

        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].name, Some("crate2".to_owned()));
        assert_eq!(dirs[0].level, LevelFilter::max());
        assert!(filter.is_none());
    }

    #[test]
    fn parse_spec_empty_level_isolated() {
        // test parse_spec with "" as log level (and the entire spec str)
        let ParseResult {
            directives: dirs,
            filter,
            ..
        } = parse_spec(""); // should be ignored
        assert_eq!(dirs.len(), 0);
        assert!(filter.is_none());
    }

    #[test]
    fn parse_spec_blank_level_isolated() {
        // test parse_spec with a white-space-only string specified as the log
        // level (and the entire spec str)
        let ParseResult {
            directives: dirs,
            filter,
            ..
        } = parse_spec("     "); // should be ignored
        assert_eq!(dirs.len(), 0);
        assert!(filter.is_none());
    }

    #[test]
    fn parse_spec_blank_level_isolated_comma_only() {
        // The spec should contain zero or more comma-separated string slices,
        // so a comma-only string should be interpreted as two empty strings
        // (which should both be treated as invalid, so ignored).
        let ParseResult {
            directives: dirs,
            filter,
            ..
        } = parse_spec(","); // should be ignored
        assert_eq!(dirs.len(), 0);
        assert!(filter.is_none());
    }

    #[test]
    fn parse_spec_blank_level_isolated_comma_blank() {
        // The spec should contain zero or more comma-separated string slices,
        // so this bogus spec should be interpreted as containing one empty
        // string and one blank string. Both should both be treated as
        // invalid, so ignored.
        let ParseResult {
            directives: dirs,
            filter,
            ..
        } = parse_spec(",     "); // should be ignored
        assert_eq!(dirs.len(), 0);
        assert!(filter.is_none());
    }

    #[test]
    fn parse_spec_blank_level_isolated_blank_comma() {
        // The spec should contain zero or more comma-separated string slices,
        // so this bogus spec should be interpreted as containing one blank
        // string and one empty string. Both should both be treated as
        // invalid, so ignored.
        let ParseResult {
            directives: dirs,
            filter,
            ..
        } = parse_spec("     ,"); // should be ignored
        assert_eq!(dirs.len(), 0);
        assert!(filter.is_none());
    }

    #[test]
    fn parse_spec_global() {
        // test parse_spec with no crate
        let ParseResult {
            directives: dirs,
            filter,
            ..
        } = parse_spec("warn,crate2=debug");
        assert_eq!(dirs.len(), 2);
        assert_eq!(dirs[0].name, None);
        assert_eq!(dirs[0].level, LevelFilter::Warn);
        assert_eq!(dirs[1].name, Some("crate2".to_owned()));
        assert_eq!(dirs[1].level, LevelFilter::Debug);
        assert!(filter.is_none());
    }

    #[test]
    fn parse_spec_global_bare_warn_lc() {
        // test parse_spec with no crate, in isolation, all lowercase
        let ParseResult {
            directives: dirs,
            filter,
            ..
        } = parse_spec("warn");
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].name, None);
        assert_eq!(dirs[0].level, LevelFilter::Warn);
        assert!(filter.is_none());
    }

    #[test]
    fn parse_spec_global_bare_warn_uc() {
        // test parse_spec with no crate, in isolation, all uppercase
        let ParseResult {
            directives: dirs,
            filter,
            ..
        } = parse_spec("WARN");
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].name, None);
        assert_eq!(dirs[0].level, LevelFilter::Warn);
        assert!(filter.is_none());
    }

    #[test]
    fn parse_spec_global_bare_warn_mixed() {
        // test parse_spec with no crate, in isolation, mixed case
        let ParseResult {
            directives: dirs,
            filter,
            ..
        } = parse_spec("wArN");
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].name, None);
        assert_eq!(dirs[0].level, LevelFilter::Warn);
        assert!(filter.is_none());
    }

    #[test]
    fn parse_spec_valid_filter() {
        let ParseResult {
            directives: dirs,
            filter,
            ..
        } = parse_spec("crate1::mod1=error,crate1::mod2,crate2=debug/abc");
        assert_eq!(dirs.len(), 3);
        assert_eq!(dirs[0].name, Some("crate1::mod1".to_owned()));
        assert_eq!(dirs[0].level, LevelFilter::Error);

        assert_eq!(dirs[1].name, Some("crate1::mod2".to_owned()));
        assert_eq!(dirs[1].level, LevelFilter::max());

        assert_eq!(dirs[2].name, Some("crate2".to_owned()));
        assert_eq!(dirs[2].level, LevelFilter::Debug);
        assert!(filter.is_some() && filter.unwrap().to_string() == "abc");
    }

    #[test]
    fn parse_spec_invalid_crate_filter() {
        let ParseResult {
            directives: dirs,
            filter,
            ..
        } = parse_spec("crate1::mod1=error=warn,crate2=debug/a.c");

        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].name, Some("crate2".to_owned()));
        assert_eq!(dirs[0].level, LevelFilter::Debug);
        assert!(filter.is_some() && filter.unwrap().to_string() == "a.c");
    }

    #[test]
    fn parse_spec_empty_with_filter() {
        let ParseResult {
            directives: dirs,
            filter,
            ..
        } = parse_spec("crate1/a*c");
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].name, Some("crate1".to_owned()));
        assert_eq!(dirs[0].level, LevelFilter::max());
        assert!(filter.is_some() && filter.unwrap().to_string() == "a*c");
    }
}
