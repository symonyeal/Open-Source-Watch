//! Event detail by rules (spec § Event detail): whether a report states an incident,
//! and the incident's ACLED sub-event type (ACLED Codebook 2023 §2), weapons, missile
//! type, target class, killed and injured counts, and the sides. The rules read the
//! English text; translation leaves some Ukrainian or Russian, whose words sit beside
//! the English ones. Measured in docs/VALIDATION.md against docs/labels/.

use std::sync::LazyLock;

use regex::Regex;

fn re(s: &str) -> Regex {
    Regex::new(&format!("(?i){s}")).unwrap()
}

macro_rules! rx {
    ($name:ident, $s:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| re($s));
    };
}

/// Number words of casualty counts.
const NUMBERS: [&str; 21] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
];
const N: &str = r"(\d+|zero|one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|twenty|a|an)";

// Something happened: a hit, a casualty, an explosion, a shoot-down, fighting.
rx!(
    WEAK,
    r"\b(kill|dead|died|death|injur|wound|casualt|victim|strik|struck|hit|attack|shell|explo|blast|destroy|damag|shot down|downed|intercept|suppress|arriv|fire|burn|blaze|clash|fight|assault|captur|liberat|repel|repuls|eliminat|liquidat|crash|sank|sink|bomb|raid|ambush)|удар|атак|обстр|вибух|взрыв|загин|погиб|поранен|ранен|збит|сбит|прил[еі]т"
);
// ... and said so concretely: a count of casualties, a hit on something, an explosion.
rx!(
    STRONG,
    r"\b(killed|kills?|killing|died|dead|injur(ed|es|ing|ies)|wounded|explosions? (in|at|rang|heard|were heard|reported|occurred)|fires? (broke|breaks)|caught fire|on fire|ablaze|struck|hit (a|an|the|two|three|several)|hits? (a|an|the|\d+|two|three|four|five|six|seven|eight|nine|ten|several|multiple)|slam(s|med)?|aimed at|under fire|(after|following) (a |the )?(massive |overnight )?(drone|missile|uav) (attacks?|strikes?)|scatter\w* explosive|advanced (near|in|to|towards)|liberated|destruction of|damag(ed|es|ing)|destroy(ed|s|ing)?|shot down|downs?|downed|intercepted|eliminat(ed|es|ing)|zeroed out|sank|sinks|sunk|under attack|arrivals?|consequences of|(attacks?|strikes?|raids?|shelling) (on|in|against|at)|(was|were|is|are|been) (attacked|hit|struck|shelled)|attacking|launch(ed|es)? [^.]{0,40}(attacks?|strikes?|missiles?|drones?)|hit (sea )?mines|strikes? (\w+ ){0,3}(plants?|refiner\w+|airfields?|depots?|ports?|bridges?|sites?|facilit\w+|infrastructure|positions))\b|знищ|уражен|не пережив"
);
// Not of the war: accidents, disasters, animals.
rx!(
    NONWAR,
    r"\b(earthquakes?|bears?|training (flight|exercises?)|during (training|landing|exercises?)|while landing|accident|hurricanes?|floods?|wildfires?)\b"
);
// A statement, plan, analysis or response, not an incident.
rx!(
    STATEMENT,
    r"^[^:]{2,40}:|\b(says?|said|claims?|warns?|urges?|calls (for|on)|accuses?|blames?|vows?|plans?|ready|will|would|could|may|might|should|talks|negotiat\w*|peace|envoy|summit|meets?|meeting|sanctions?|proposals?|deal|ceasefire|truce|aid|budget|contract|exercises?|interceptors|capabilit\w*|analysis|why|how|podcast|programme?|interview|losses|estimates?|rethinks|suspects|blocking|surveillance|expels?|bans?)\b|\?"
);
// A warning or a track, not a hit: "Jet UAVs in the direction of Korosten".
rx!(
    NOTICE,
    r"\b(threat of|danger|alarm|heading|course|in the direction|direction of|towards?|➡)"
);

rx!(
    DRONE,
    r"\b(drones?|uavs?|unmanned|shaheds?|geran\w*|geraniums?|fpv|lancets?|kamikaze)\b|бпла|дрон|шахед"
);
rx!(
    MISSILE,
    r"\b(missiles?|ballistic|cruise|iskander\w*|kh-\d+|kalibr|kinzhal|s-300|s-400|storm shadow|atacms|neptune|oreshnik|rockets?)\b|ракет"
);
rx!(
    GLIDE,
    r"\b(glide bombs?|guided (aerial )?bombs?|aerial bombs?|kabs?|cabs?|umpk|fab-\d+)\b|\bкаб"
);
rx!(
    ARTILLERY,
    r"\b(shelling|shelled|artillery|mlrs|grad|uragan|smerch|mortars?|howitzers?|self-propelled gun|gvozdika|tank fire)\b|обстр"
);
/// Missile types, most specific first; else "ballistic" or "cruise".
const MISSILES: [(&str, &str); 15] = [
    (r"iskander-m", "Iskander-M"),
    (r"iskander", "Iskander"),
    (r"kh-?101", "Kh-101"),
    (r"kh-?22", "Kh-22"),
    (r"kh-?59", "Kh-59"),
    (r"kalibr", "Kalibr"),
    (r"kinzhal", "Kinzhal"),
    (r"s-300", "S-300"),
    (r"s-400", "S-400"),
    (r"banderol", "Banderol"),
    (r"storm shadow", "Storm Shadow"),
    (r"atacms", "ATACMS"),
    (r"neptune", "Neptune"),
    (r"oreshnik", "Oreshnik"),
    (r"flamingo", "Flamingo"),
];

rx!(
    DISRUPTED,
    r"\b(shot down|downed|intercepted|destroyed|suppressed|repelled|repulsed|neutrali[sz]ed)\b"
);
rx!(
    HIT,
    r"\b(hit|hits|struck|damag\w*|killed|kills|injur\w*|fires?|explosions?|arriv\w*|consequences)\b"
);
rx!(IED, r"\b(mines?|ied|explosive device|blown up|landmine)\b");
rx!(
    BATTLE,
    r"\b(clash\w*|fighting|assault\w*|battles?|positions|advanc\w*|captur\w*|liberat\w*|repel\w*|landing force|infantry)\b"
);

/// Target classes and their words; a site's kind decides first (`SITE_TARGET`).
const TARGETS: [(&str, &str); 5] = [
    (
        "energy",
        r"\b(energy|power (grid|lines?|supply|plant|station|facilit\w+)|substations?|electricity|blackouts?|refiner\w*|oil|gas (facilit\w+|infrastructure|production)|fuel|thermal|chp\w*|tpp|heat(ing)? (plant|station))\b",
    ),
    (
        "military",
        r"\b(airfields?|air ?bases?|planes?|helicopters?|fighters?|bombers?|su-\d+|tu-\d+|military (facilit\w+|targets?|objects?|bases?|units?|equipment|enterprises?)|soldiers|servicem[ae]n|positions|strongholds?|deployment points?|control points?|dugouts?|nests|scouts|defenders|infantry|brigade|battalion|equipment|vehicles|armou?red|tanks|launchers?|pantsir|ammunition|shells|arsenal|command post|headquarters|radars?|air defen[cs]e (system|complex)|defen[cs]e (industry|plant|enterprise|factor\w+)|missile (plant|production|component)|warehouses? of|depots?)\b|склад",
    ),
    (
        "infrastructure",
        r"\b(bridges?|ports?|railway|rail|stations?|locomotives?|dams?|roads?|highways?|infrastructure|logistics|telecom\w*|towers?|water|plants?|factory|enterprises?|industrial|warehouses?|data cent(er|re)|parking|завод)\b",
    ),
    (
        "civilian",
        r"\b(residential|homes?|houses?|apartments?|high-rise|buildings?|civilians?|markets?|shops?|shopping|malls?|hypermarkets?|stores?|schools?|universit\w+|hospitals?|kindergartens?|dormitor\w+|social facilit\w+|business cent(er|re)|cars?|bus|trains?|ambulances?|paramedics|medics|rescuers|volunteers?|church|cafe|hotel|retail|merchant|commercial (ship|vessel)|cargo ship|container ship|ships?|vessels?|sailors?)\b",
    ),
    // People last: the victims, not what was aimed at, unless nothing else is named.
    (
        "civilian",
        r"\b(people|residents|children|child|woman|man)\b",
    ),
];
// An industrial target that makes arms is military ("Proton plant ... components for
// missiles").
rx!(
    DEFENCE,
    r"\b(missiles?|military(-industrial)?|defen[cs]e (industry|industrial|enterprise)|arms|ammunition|explosives|rocket (fuel|propellant))\b"
);
/// The target class of a site kind (process::place `SITE_WORDS`).
const SITE_TARGET: [(&str, &str); 10] = [
    ("refinery", "energy"),
    ("oil_depot", "energy"),
    ("power_plant", "energy"),
    ("nuclear_plant", "energy"),
    ("substation", "energy"),
    ("airfield", "military"),
    ("port", "infrastructure"),
    ("bridge", "infrastructure"),
    ("dam", "infrastructure"),
    ("plant", "infrastructure"),
];

rx!(
    BY_RU,
    r"\b(russian (forces|troops|army|military|drones?|missiles?|strikes?|attacks?|shelling|aviation|uavs?|shaheds?|jet|ballistic|federation (struck|attacked|hit|launched))|russia('s)? (attacks?|attacked|strikes?|struck|hits?|launch\w*|shell\w*|pounds?|bombs?|targets?)|russians (attack\w*|strike|struck|hit|shell\w*|launch\w*)|by russia|rashists?|rachists?|occupiers|rusnya|the enemy (attacked|struck|hit|launched|used))\b|росі[яйї]\w* (атак|удар)"
);
rx!(
    BY_UA,
    r"\b(ukrainian (forces|troops|army|military|drones?|missiles?|strikes?|attacks?|uavs?|armed forces|defen[cs]e forces|navy|special)|ukraine('s)? (attacks?|attacked|strikes?|struck|hits?|launch\w*|targets?)|armed forces of ukraine|by the afu|afu|defen[cs]e forces|sbu|general staff|by ukrain\w*|ukrainian hook|partisans)\b|сили оборони|зсу|всу"
);

/// Fields of one report's incident.
#[derive(Debug, Default, PartialEq)]
pub struct Detail {
    pub event: bool,
    pub sub_event: Option<&'static str>,
    pub weapon: Vec<&'static str>,
    pub missile: Option<&'static str>,
    pub target: Option<&'static str>,
    pub killed: Option<i32>,
    pub injured: Option<i32>,
    pub actor1: Option<&'static str>,
    pub actor2: Option<&'static str>,
}

fn number(s: &str) -> Option<i32> {
    let s = s.to_lowercase();
    match s.as_str() {
        "a" | "an" => Some(1),
        "dozen" | "a dozen" => Some(12),
        _ => s
            .parse()
            .ok()
            .or_else(|| NUMBERS.iter().position(|x| *x == s).map(|k| k as i32)),
    }
}

rx!(
    NONE_HURT,
    r"\b(no (casualties|victims|injur\w+|one was (killed|injured|hurt))|without (casualties|victims)|no (people|one) (were|was) (killed|injured|hurt))\b"
);

/// The largest count of people a casualty word w states in s ("13 people were
/// killed", "kills 2", "the number injured has increased to 34"); 0 if the text says
/// nobody was hurt.
fn counters(passive: &str, active: &str) -> Vec<Regex> {
    [
        format!(
            r"\b{N}\s+(?:[\w-]+,?\s+){{0,4}}(?:were |was |have been |has been |had been |are )?(?:{passive})\b"
        ),
        format!(
            r"\b(?:{active})\s+(?:at least |up to |about |nearly |over )?(\d+|zero|one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|thirteen|fourteen|fifteen|sixteen|seventeen|eighteen|nineteen|twenty|a dozen)\b"
        ),
        format!(
            r"\b(?:{passive}|{active})\b[^.]{{0,90}}\b(?:rose|risen|rises|increased|grew|reached)\s+to\s+{N}\b"
        ),
    ]
    .iter()
    .map(|p| re(p))
    .collect()
}

fn count(s: &str, P: &[Regex]) -> Option<i32> {
    P.iter()
        .flat_map(|p| {
            p.captures_iter(s)
                .filter_map(|c| number(c.get(1)?.as_str()))
                .collect::<Vec<_>>()
        })
        .filter(|&n| n < 10_000)
        .max()
        .or_else(|| NONE_HURT.is_match(s).then_some(0))
}

/// The first of rules R (name, regex) to match s, by position.
fn first<'r>(s: &str, R: impl Iterator<Item = (&'static str, &'r Regex)>) -> Option<&'static str> {
    R.filter_map(|(k, r)| Some((r.find(s)?.start(), k)))
        .min()
        .map(|x| x.1)
}

/// The detail of report text s, given the kind of the site it names and the
/// country (ISO alpha-2) of its place, if known. A report states an incident when it
/// is not of the war's background (`NONWAR`) and says something concrete happened
/// (`STRONG`), or something happened without the marks of a statement or a warning
/// (`WEAK`, `STATEMENT`, `NOTICE`), or names a weapon: a weapon named keeps reports
/// such as "Russia attacks Ukraine with 433 drones" that the words alone would drop.
pub fn detail(s: &str, site: Option<&str>, country: Option<&str>) -> Detail {
    let W: [(&str, &Regex); 4] = [
        ("drone", &DRONE),
        ("missile", &MISSILE),
        ("glide_bomb", &GLIDE),
        ("artillery", &ARTILLERY),
    ];
    let mut weapon: Vec<(usize, &str)> = W
        .iter()
        .filter_map(|(k, r)| Some((r.find(s)?.start(), *k)))
        .collect();
    weapon.sort();
    let weapon: Vec<&str> = weapon.into_iter().map(|x| x.1).collect();
    let event = !NONWAR.is_match(s)
        && (STRONG.is_match(s)
            || (WEAK.is_match(s) && !STATEMENT.is_match(s) && !NOTICE.is_match(s))
            || !weapon.is_empty());
    if !event {
        return Detail::default();
    }
    let missile = weapon
        .contains(&"missile")
        .then(|| {
            static M: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
                MISSILES
                    .iter()
                    .map(|(p, k)| (re(p), *k))
                    .chain([(re("ballistic"), "ballistic"), (re("cruise"), "cruise")])
                    .collect()
            });
            M.iter().find(|(r, _)| r.is_match(s)).map(|x| x.1)
        })
        .flatten();
    let sub_event = if DISRUPTED.is_match(s) && !HIT.is_match(s) {
        "Disrupted weapons use"
    } else if IED.is_match(s) {
        "Remote explosive/landmine/IED"
    } else {
        match weapon.first() {
            Some(&("drone" | "glide_bomb")) => "Air/drone strike",
            Some(_) => "Shelling/artillery/missile attack",
            None if BATTLE.is_match(s) => "Armed clash",
            None => "Shelling/artillery/missile attack",
        }
    };
    static T: LazyLock<Vec<(&str, Regex)>> =
        LazyLock::new(|| TARGETS.iter().map(|(k, p)| (*k, re(p))).collect());
    static KILLED: LazyLock<Vec<Regex>> =
        LazyLock::new(|| counters("killed|dead|died|dies", "kills?|killing|killed|death toll"));
    static INJURED: LazyLock<Vec<Regex>> =
        LazyLock::new(|| counters("injured|wounded|hurt", "injures?|injuring|injured|wounding"));
    let (killed, injured) = (count(s, &KILLED), count(s, &INJURED));
    // A site's kind first, then the first class named; an arms maker is military, and
    // people hurt with nothing else named are civilians.
    let target = site
        .and_then(|k| SITE_TARGET.iter().find(|x| x.0 == k).map(|x| x.1))
        .or_else(|| first(s, T[..4].iter().map(|(k, r)| (*k, r))))
        .or_else(|| T[4].1.is_match(s).then_some(T[4].0))
        .map(|t| {
            if t == "infrastructure" && DEFENCE.is_match(s) {
                "military"
            } else {
                t
            }
        })
        .or_else(|| (killed > Some(0) || injured > Some(0)).then_some("civilian"));
    // The side named first as a subject: a side after "of" or "against" is the object
    // ("positions of the Ukrainian Armed Forces").
    let lower = s.to_lowercase();
    let object = |k: usize| {
        let b = lower[..k].trim_end();
        [
            "of",
            "of the",
            "against",
            "against the",
            "at",
            "at the",
            "on",
            "on the",
        ]
        .iter()
        .any(|w| b.ends_with(&format!(" {w}")))
    };
    let by = [("Russia", &*BY_RU), ("Ukraine", &*BY_UA)]
        .into_iter()
        .filter_map(|(k, r)| Some((r.find_iter(s).find(|m| !object(m.start()))?.start(), k)))
        .min()
        .map(|x| x.1);
    let place = match country {
        Some("RU") => Some("Russia"),
        Some("UA") => Some("Ukraine"),
        Some(_) => Some("other"),
        None => None,
    };
    let other = |a: &str| if a == "Russia" { "Ukraine" } else { "Russia" };
    let (actor1, actor2) = match (by, place) {
        (Some(a), Some(p)) if p != a => {
            (Some(a), Some(if p == "other" { "other" } else { other(a) }))
        }
        (Some(a), _) => (Some(a), Some(other(a))),
        (None, Some(p)) if p != "other" => (Some(other(p)), Some(p)),
        _ => (None, None),
    };
    Detail {
        event,
        sub_event: Some(sub_event),
        weapon,
        missile,
        target,
        killed,
        injured,
        actor1,
        actor2,
    }
}
