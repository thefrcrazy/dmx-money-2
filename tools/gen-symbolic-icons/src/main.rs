//! Convertit les SVG Lucide en icônes symboliques GTK.
//!
//! GTK recolore une icône symbolique en forçant `fill` sur les formes : les icônes Lucide,
//! dessinées au trait, deviendraient des taches pleines. On transforme donc chaque trait en
//! surface fermée avant d'écrire le fichier, ce qui rend l'icône correcte sur toutes les
//! versions de GTK, avec la couleur du thème comme avec les couleurs de comptes.
//!
//! Usage : gen-symbolic-icons <dossier source> <dossier destination>

use std::path::Path;

use tiny_skia_path::{LineCap, LineJoin, PathStroker, Stroke};

fn main() {
    let mut arguments = std::env::args().skip(1);
    let (Some(source), Some(target)) = (arguments.next(), arguments.next()) else {
        eprintln!("usage: gen-symbolic-icons <dossier source> <dossier destination>");
        std::process::exit(2);
    };
    let source = Path::new(&source);
    let target = Path::new(&target);
    if let Err(error) = std::fs::create_dir_all(target) {
        eprintln!("impossible de créer {}: {error}", target.display());
        std::process::exit(1);
    }

    let mut entries: Vec<_> = std::fs::read_dir(source)
        .unwrap_or_else(|error| panic!("lecture de {}: {error}", source.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().map(|extension| extension == "svg").unwrap_or(false))
        .collect();
    entries.sort();

    let mut count = 0usize;
    for path in entries {
        let lucide = path.file_stem().unwrap().to_string_lossy().to_string();
        let data = std::fs::read(&path).expect("lecture de l'icône");
        let document = match convert(&data) {
            Ok(document) => document,
            Err(error) => {
                eprintln!("{lucide}: {error}");
                std::process::exit(1);
            }
        };
        let name = format!("{}.svg", icon_name(&lucide));
        std::fs::write(target.join(&name), document).expect("écriture de l'icône");
        count += 1;
    }
    println!("==> {count} icônes symboliques dans {}", target.display());
}

/// « Wallet » → « dmx-wallet-symbolic » (même règle que `icons.rs`).
fn icon_name(lucide: &str) -> String {
    let mut kebab = String::new();
    for (index, character) in lucide.chars().enumerate() {
        if character.is_uppercase() && index > 0 {
            kebab.push('-');
        }
        kebab.extend(character.to_lowercase());
    }
    format!("dmx-{kebab}-symbolic")
}

fn convert(data: &[u8]) -> Result<String, String> {
    let tree = usvg::Tree::from_data(data, &usvg::Options::default()).map_err(|error| error.to_string())?;
    let size = tree.size();
    let mut paths = Vec::new();
    collect(tree.root(), &mut paths);
    if paths.is_empty() {
        return Err("aucune forme convertie".to_string());
    }

    let mut document = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n",
        width = size.width() as i32,
        height = size.height() as i32
    );
    for path in paths {
        document.push_str(&format!(
            "  <path class=\"foreground-fill\" fill=\"#000000\" d=\"{path}\"/>\n"
        ));
    }
    document.push_str("</svg>\n");
    Ok(document)
}

/// Parcourt l'arbre et transforme chaque trait en surface (les remplissages sont conservés).
fn collect(group: &usvg::Group, out: &mut Vec<String>) {
    for node in group.children() {
        match node {
            usvg::Node::Group(child) => collect(child, out),
            usvg::Node::Path(path) => {
                if path.fill().is_some() {
                    out.push(path_data(path.data()));
                }
                if let Some(stroke) = path.stroke() {
                    let outline = PathStroker::new().stroke(path.data(), &to_stroke(stroke), 1.0);
                    match outline {
                        Some(outline) => out.push(path_data(&outline)),
                        None => eprintln!("trait non convertible, forme ignorée"),
                    }
                }
            }
            _ => {}
        }
    }
}

fn to_stroke(stroke: &usvg::Stroke) -> Stroke {
    Stroke {
        width: stroke.width().get(),
        miter_limit: stroke.miterlimit().get(),
        line_cap: match stroke.linecap() {
            usvg::LineCap::Butt => LineCap::Butt,
            usvg::LineCap::Round => LineCap::Round,
            usvg::LineCap::Square => LineCap::Square,
        },
        line_join: match stroke.linejoin() {
            usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => LineJoin::Miter,
            usvg::LineJoin::Round => LineJoin::Round,
            usvg::LineJoin::Bevel => LineJoin::Bevel,
        },
        dash: None,
    }
}

/// Sérialise un chemin en attribut `d`, avec deux décimales (suffisant à 24 px).
fn path_data(path: &tiny_skia_path::Path) -> String {
    use tiny_skia_path::PathSegment;
    let mut data = String::new();
    for segment in path.segments() {
        match segment {
            PathSegment::MoveTo(point) => data.push_str(&format!("M{} {}", round(point.x), round(point.y))),
            PathSegment::LineTo(point) => data.push_str(&format!("L{} {}", round(point.x), round(point.y))),
            PathSegment::QuadTo(control, point) => data.push_str(&format!(
                "Q{} {} {} {}",
                round(control.x),
                round(control.y),
                round(point.x),
                round(point.y)
            )),
            PathSegment::CubicTo(first, second, point) => data.push_str(&format!(
                "C{} {} {} {} {} {}",
                round(first.x),
                round(first.y),
                round(second.x),
                round(second.y),
                round(point.x),
                round(point.y)
            )),
            PathSegment::Close => data.push('Z'),
        }
        data.push(' ');
    }
    data.trim_end().to_string()
}

fn round(value: f32) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    let text = format!("{rounded}");
    if text == "-0" {
        "0".to_string()
    } else {
        text
    }
}
