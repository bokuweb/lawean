//! 法令の各文の主体・行為: JEWEL_GINZA_BUNDLE=... cargo run -p lawean-nlp --example subjects -- <xml> [--all]
//! 効果種別が義務・禁止・可能・不能の文のうち、主述語の主語が取れた割合を出す
use lawean_extract::candidate::Field;
use lawean_extract::effect::{classify, EffectKind};
use lawean_nlp::Parser;
use lawean_source::parse_response;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let doc = parse_response(&std::fs::read_to_string(&args[0]).unwrap()).unwrap();
    let all = args.iter().any(|a| a == "--all");
    let p = Parser::from_env().unwrap();
    let (mut norm, mut with_subj) = (0, 0);
    let t0 = std::time::Instant::now();
    for g in doc.sentence_groups() {
        for s in &g.sentences {
            let text = s.sentence.plain_text();
            let (kind, _) = classify(&text);
            if !matches!(
                kind,
                EffectKind::Obligation
                    | EffectKind::Prohibition
                    | EffectKind::Can
                    | EffectKind::CanNot
            ) {
                continue;
            }
            norm += 1;
            let cs = p.candidates(&s.sentence.stable_id, &text).unwrap();
            let root_subj = cs.iter().find(|c| {
                c.field == Field::Subject
                    && c.reason != "ginza:clause"
                    && c.confidence != lawean_extract::candidate::Confidence::Low
            });
            let act = cs
                .iter()
                .find(|c| c.field == Field::Act && c.reason == "ginza:root");
            if root_subj.is_some() {
                with_subj += 1;
            }
            if all || root_subj.is_none() {
                println!(
                    "{} [{kind:?}] subj={:?} ({:?}) act={:?} | {}",
                    s.sentence.stable_id.0.rsplit("/main/").next().unwrap_or(""),
                    root_subj.map(|c| c.raw.as_str()),
                    root_subj.and_then(|c| c.role.as_deref()),
                    act.map(|c| c.raw.as_str()),
                    text.chars().take(60).collect::<String>()
                );
            }
        }
    }
    println!(
        "normative sentences {norm}, with root subject {with_subj} ({:.0}%), {:.1}s",
        100.0 * with_subj as f64 / norm.max(1) as f64,
        t0.elapsed().as_secs_f64()
    );
}
