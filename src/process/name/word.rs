#![allow(clippy::trivial_regex)]
use regex::Regex;

// create replacement tuple
macro_rules! crtpl {
    ($a:expr, $b:expr) => {
        (Regex::new($a).unwrap(), $b)
    };
}

lazy_static! {
    // regexp source: https://gist.github.com/tbrianjones/ba0460cc1d55f357e00b

    static ref SINGULAR_LIST: [(Regex, &'static str); 28] = [
        crtpl!("(quiz)zes$", "${1}"),
        crtpl!("(matr)ices$", "${1}ix"),
        crtpl!("(vert|ind)ices$", "${1}ex"),
        crtpl!("^(ox)en$", "${1}"),
        crtpl!("(alias)es$", "${1}"),
        crtpl!("(octop|vir)i$", "${1}us"),
        crtpl!("(cris|ax|test)es$", "${1}is"),
        crtpl!("(shoe)s$", "${1}"),
        crtpl!("(o)es$", "${1}"),
        crtpl!("(bus)es$", "${1}"),
        crtpl!("([m|l])ice$", "${1}ouse"),
        crtpl!("(x|ch|ss|sh)es$", "${1}"),
        crtpl!("(m)ovies$", "${1}ovie"),
        crtpl!("(s)eries$", "${1}eries"),
        crtpl!("([^aeiouy]|qu)ies$", "${1}y"),
        crtpl!("([lr])ves$", "${1}f"),
        crtpl!("(tive)s$", "${1}"),
        crtpl!("(hive)s$", "${1}"),
        crtpl!("(li|wi|kni)ves$", "${1}fe"),
        crtpl!("(shea|loa|lea|thie)ves$", "${1}f"),
        crtpl!("(^analy)ses$", "${1}sis"),
        crtpl!("((a)naly|(b)a|(d)iagno|(p)arenthe|(p)rogno|(s)ynop|(t)he)ses$", "${1}${2}sis"),
        crtpl!("([ti])a$", "${1}um"),
        crtpl!("(n)ews$", "${1}ews"),
        crtpl!("(h|bl)ouses$", "${1}ouse"),
        crtpl!("(corpse)s$", "${1}"),
        crtpl!("(us)es$", "${1}"),
        crtpl!("s$", "")
    ];

    static ref PLURAL_LIST: [(Regex, &'static str); 19] = [
        crtpl!("(quiz)$", "${1}zes"),
        crtpl!("^(ox)$", "${1}en"),
        crtpl!("([m|l])ouse$", "${1}ice"),
        crtpl!("(matr|vert|ind)ix|ex$", "${1}ices"),
        crtpl!("(x|ch|ss|sh)$", "${1}es"),
        crtpl!("([^aeiouy]|qu)y$", "${1}ies"),
        crtpl!("(hive)$", "${1}s"),
        crtpl!("(?:([^f])fe|([lr])f)$", "${1}${2}ves"),
        crtpl!("(shea|lea|loa|thie)f$", "${1}ves"),

        crtpl!("sis$", "ses"),
        crtpl!("([ti])um$", "${1}a"),
        crtpl!("(tomat|potat|ech|her|vet)o$", "${1}oes"),
        crtpl!("(bu)s$", "${1}ses"),
        crtpl!("(alias)$", "${1}es"),
        crtpl!("(octop)us$", "${1}i"),
        crtpl!("(ax|test)is$", "${1}es"),
        crtpl!("(us)$", "${1}es"),
        crtpl!("s$", "s"),
        crtpl!("$", "s")
    ];
}

pub fn singularize(word: String) -> String {
    for (re, replacement) in SINGULAR_LIST.iter() {
        if re.is_match(&word) {
            return re.replace_all(&word, *replacement).to_string();
        }
    }

    word
}

pub fn is_plurar(word: String) -> bool {
    let plurar_form = pluralize(word.clone());

    if plurar_form == word {
        return true;
    }

    false
}

pub fn pluralize(word: String) -> String {
    for (re, replacement) in PLURAL_LIST.iter() {
        if re.is_match(&word) {
            return re.replace_all(&word, *replacement).to_string();
        }
    }

    word
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case( "user".to_string(), "users".to_string() )]
    #[test_case( "user-group".to_string(), "user-groups".to_string() )]
    #[test_case( "bus".to_string(), "buses".to_string() )]
    #[test_case( "alias".to_string(), "aliases".to_string() )]
    fn test_pluralize(word: String, expected: String) {
        assert_eq!(pluralize(word), expected);
    }

    #[test_case( "users".to_string(), "user".to_string() )]
    #[test_case( "user-groups".to_string(), "user-group".to_string() )]
    #[test_case( "buses".to_string(), "bus".to_string() )]
    #[test_case( "aliases".to_string(), "alias".to_string() )]
    #[test_case( "fixes".to_string(), "fix".to_string() )]
    fn test_singularize(word: String, expected: String) {
        assert_eq!(singularize(word), expected);
    }
}
