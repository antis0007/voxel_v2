//! Text contract: world-shader shadow lookups stay in the render-origin frame.
//!
//! Cascade matrices map render-origin metres. A merge that multiplies one with
//! an absolute position, or feeds `sample_sun_shadow` absolute metres, still
//! compiles and looks right near spawn, so only a text check catches it.

const SAMPLER: &str = "sample_sun_shadow";
const RECEIVER: &str = "cam.eye_from_origin.xyz+relative_pos";

/// Blanks comments, keeping code byte offsets stable.
fn strip_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = bytes.to_vec();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'/' && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                out[i] = b' ';
                i += 1;
            }
        } else if bytes[i] == b'/' && bytes[i + 1] == b'*' {
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                if bytes[i] != b'\n' {
                    out[i] = b' ';
                }
                i += 1;
            }
            out[i] = b' ';
            if i + 1 < out.len() {
                out[i + 1] = b' ';
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    // Only ASCII bytes were replaced with spaces, so the text stays UTF-8.
    String::from_utf8_lossy(&out).into_owned()
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Byte offsets of `word` as a whole identifier.
fn word_offsets(source: &str, word: &str) -> Vec<usize> {
    let bytes = source.as_bytes();
    source
        .match_indices(word)
        .map(|(at, _)| at)
        .filter(|&at| {
            let before = at == 0 || !is_ident(bytes[at - 1]);
            let end = at + word.len();
            before && (end == bytes.len() || !is_ident(bytes[end]))
        })
        .collect()
}

/// Every function's name and byte span, `fn` to closing brace.
fn functions(source: &str) -> Vec<(String, std::ops::Range<usize>)> {
    let bytes = source.as_bytes();
    let mut found = Vec::new();
    for at in word_offsets(source, "fn") {
        let rest = &source[at + 2..];
        let name: String = rest.trim_start().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        let Some(open) = source[at..].find('{').map(|o| o + at) else { continue };
        let mut depth = 0;
        for (i, &c) in bytes.iter().enumerate().skip(open) {
            depth += i32::from(c == b'{') - i32::from(c == b'}');
            if depth == 0 {
                found.push((name, at..i + 1));
                break;
            }
        }
    }
    found
}

/// Name of the function whose span holds `at`, if any.
fn enclosing(fns: &[(String, std::ops::Range<usize>)], at: usize) -> Option<&(String, std::ops::Range<usize>)> {
    fns.iter().find(|(_, span)| span.contains(&at))
}

/// Whether the access path at byte `at` is a `*` operand.
fn multiplied(source: &str, at: usize, len: usize) -> bool {
    let bytes = source.as_bytes();
    let path = |c: u8| is_ident(c) || c == b'.' || c == b'[' || c == b']';
    let mut start = at;
    while start > 0 && path(bytes[start - 1]) {
        start -= 1;
    }
    let before = source[..start].trim_end().as_bytes().last().copied();
    let after = source[at + len..].trim_start().as_bytes().first().copied();
    before == Some(b'*') || after == Some(b'*')
}

/// Locals bound to a bare `...light_view_proj` access inside `body`.
fn matrix_aliases(body: &str) -> Vec<String> {
    body.split([';', '{', '}'])
        .filter_map(|statement| {
            let statement = statement.trim();
            let rest = statement.strip_prefix("let ").or_else(|| statement.strip_prefix("var "))?;
            let (name, value) = rest.split_once('=')?;
            let name = name.split(':').next()?.trim();
            value.trim().ends_with("light_view_proj").then(|| name.to_string())
        })
        .collect()
}

/// Cascade-matrix products outside `sample_sun_shadow`, as (function, offset).
fn stray_matrix_products(source: &str) -> Vec<(String, usize)> {
    let fns = functions(source);
    let owner = |at| enclosing(&fns, at).map_or_else(|| "<global>".to_string(), |(name, _)| name.clone());
    let mut stray: Vec<(String, usize)> = word_offsets(source, "light_view_proj")
        .into_iter()
        .filter(|&at| multiplied(source, at, "light_view_proj".len()))
        .map(|at| (owner(at), at))
        .collect();
    for (name, span) in &fns {
        let body = &source[span.clone()];
        for alias in matrix_aliases(body) {
            for at in word_offsets(body, &alias) {
                if multiplied(body, at, alias.len()) {
                    stray.push((name.clone(), span.start + at));
                }
            }
        }
    }
    stray.retain(|(name, _)| name != SAMPLER);
    stray
}

/// First argument of each `sample_sun_shadow` call that is not origin-frame.
fn bad_receivers(source: &str) -> (usize, Vec<String>) {
    let fns = functions(source);
    let squeeze = |text: &str| text.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    let mut calls = 0;
    let mut bad = Vec::new();
    for at in word_offsets(source, SAMPLER) {
        let after = &source[at + SAMPLER.len()..];
        if !after.trim_start().starts_with('(') || source[..at].trim_end().ends_with("fn") {
            continue;
        }
        calls += 1;
        let args = &after[after.find('(').unwrap_or(0) + 1..];
        let mut depth = 0;
        let end = args
            .char_indices()
            .find(|&(_, c)| {
                depth += i32::from(c == '(') - i32::from(c == ')');
                depth < 0 || (depth == 0 && c == ',')
            })
            .map_or(args.len(), |(i, _)| i);
        let first = squeeze(&args[..end]);
        if first == RECEIVER {
            continue;
        }
        let bound = enclosing(&fns, at).is_some_and(|(_, span)| {
            source[span.clone()].split(';').any(|statement| {
                let statement = squeeze(statement);
                statement.ends_with(&format!("let{first}={RECEIVER}"))
                    || statement.ends_with(&format!("var{first}={RECEIVER}"))
            })
        });
        if !bound {
            bad.push(first);
        }
    }
    (calls, bad)
}

#[test]
fn shadow_frame_contract_world_shader() {
    let source = strip_comments(&crate::shader_sources::world_shader_source());
    let stray = stray_matrix_products(&source);
    let context = |at: usize| source[at.saturating_sub(60)..(at + 60).min(source.len())].to_string();
    assert!(
        stray.is_empty(),
        "cascade matrix multiplied outside {SAMPLER}: {:?}",
        stray.iter().map(|(name, at)| (name, context(*at))).collect::<Vec<_>>()
    );
    let (calls, bad) = bad_receivers(&source);
    assert!(calls > 0, "no {SAMPLER} call found");
    assert!(bad.is_empty(), "{SAMPLER} receivers not in the render-origin frame: {bad:?}");
}

#[test]
fn shadow_frame_contract_checker_rejects_absolute_frames() {
    let ok = "fn sample_sun_shadow(r: vec3<f32>) -> f32 { let p = c.light_view_proj * vec4<f32>(r, 1.0); return p.x; }
        fn main() { let a = sample_sun_shadow(cam.eye_from_origin.xyz + relative_pos);
            let q = cam.eye_from_origin.xyz + relative_pos; let b = sample_sun_shadow(q);
            let m = cascade.light_view_proj; let s = length(m[0].xyz); }";
    assert!(stray_matrix_products(ok).is_empty());
    assert_eq!(bad_receivers(ok), (2, vec![]));

    let absolute_call = "fn main() { let a = sample_sun_shadow(world_pos, 1.0); }";
    assert_eq!(bad_receivers(absolute_call), (1, vec!["world_pos".to_string()]));
    let product = "fn main() { let p = shadow.cascades[0].light_view_proj * vec4<f32>(world_pos, 1.0); }";
    assert_eq!(stray_matrix_products(product).len(), 1);
    let aliased = "fn main() { let m = shadow.cascades[1].light_view_proj; let p = m * vec4<f32>(world_pos, 1.0); }";
    assert_eq!(stray_matrix_products(aliased).len(), 1);
    let commented = strip_comments("fn main() { // light_view_proj * p\n let a = 1.0; }");
    assert!(stray_matrix_products(&commented).is_empty());
}
