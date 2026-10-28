//! Bøying med låst buelengde (issue #4, del 5).
//!
//! Et uttrekk med lengde S fra et startpunkt i retningen T bøyes til en rett
//! del, en sirkelbue med vinkel θ og radius r, og en rett rest. Summen av de
//! tre lengdene er alltid S, så verken bøying eller endring av radius
//! forlenger eller krymper senterlinjen. Retningen til bøyen er en vilkårlig
//! enhetsvektor vinkelrett på T, og θ går fra 0 til 180°. Sammen dekker de
//! alle enderetninger på en hel kule. Radius null gir en skarp knekk, og en
//! radius som ikke får plass, begrenses til S/θ.
//!
//! Buen tilnærmes med kubiske Bézier-segmenter på høyst 30°. Avviket i
//! buelengde er da under 1e-6 av S (se testene). Håndtakenes lengde er en
//! følge av radien og vinkelen; den er ikke selve bøyeradien.
//!
//! En bøy er høyst 180°. For å bøye videre legges et nytt punkt inn i
//! senterlinjen, og resten fra det punktet bøyes på nytt (`bend_from`).
//!
//! Bøyen starter på midten av kurven, og scroll utvider den (avklart med
//! Sivert 28. oktober 2026). Midten av buen ligger derfor i halve lengden
//! (`DEFAULT_PIVOT`), og en større radius gjør buen lengre like langt til
//! begge sider. Når buen når en ende, skyves den inn igjen i stedet for å
//! forlenge kurven.

use crate::curve::{add, scale};
use crate::mesh::{dot, normalize, sub, Vec3};
use crate::skeleton::{Anchor, AnchorKind, WorldSpline};
use std::f64::consts::PI;

/// Største vinkel per Bézier-segment i buen.
const MAX_PIECE: f64 = PI / 6.0;

/// Dokumentert toleranse for bevart buelengde, relativt til S.
pub const LENGTH_TOLERANCE: f64 = 1e-6;

/// Bøyen starter på midten av kurven.
pub const DEFAULT_PIVOT: f64 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bend {
    /// Retningen enden bøyes mot, vinkelrett på startretningen.
    pub direction: Vec3,
    /// Bøyevinkel i radianer, 0 til π.
    pub angle: f64,
    /// Bøyeradius i mm. Null gir en skarp knekk.
    pub radius: f64,
    /// Andel av lengden der midten av buen ligger, 0 til 1.
    pub pivot: f64,
}

impl Bend {
    /// Bøy med midten av buen på midten av kurven. Radius styres med scroll.
    pub fn at_middle(direction: Vec3, angle: f64, radius: f64) -> Bend {
        Bend {
            direction,
            angle,
            radius,
            pivot: DEFAULT_PIVOT,
        }
    }
}

/// Største radius som får plass når buen skal ha vinkelen `angle`.
pub fn max_radius(length: f64, angle: f64) -> f64 {
    if angle <= 1e-12 {
        f64::INFINITY
    } else {
        length / angle
    }
}

/// Vinkelsnapping til hele trinn, for eksempel 15° eller 30°.
pub fn snap_angle(angle: f64, step: f64) -> f64 {
    if step <= 0.0 {
        angle
    } else {
        (angle / step).round() * step
    }
}

/// Bøyen slik den faktisk blir: vinkel i [0, π] og radius begrenset.
pub fn effective(bend: &Bend, length: f64) -> Bend {
    let angle = bend.angle.clamp(0.0, PI);
    Bend {
        angle,
        radius: bend.radius.max(0.0).min(max_radius(length, angle)),
        pivot: bend.pivot.clamp(0.0, 1.0),
        ..*bend
    }
}

/// Ankerpunktene for et uttrekk med lengde `length` fra `start` i retningen
/// `tangent`, bøyd som angitt.
pub fn bent_anchors(
    start: Vec3,
    tangent: Vec3,
    length: f64,
    bend: &Bend,
) -> Result<Vec<Anchor>, String> {
    if !(length.is_finite() && length > 0.0) {
        return Err("Uttrekket må ha en lengde".into());
    }
    let t = normalize(tangent);
    if (crate::mesh::length(t) - 1.0).abs() > 1e-9 {
        return Err("Startretningen mangler".into());
    }
    let b = effective(bend, length);
    let n = sub(b.direction, scale(t, dot(b.direction, t)));
    if b.angle < 1e-9 || crate::mesh::length(n) < 1e-9 {
        let h = scale(t, length / 3.0);
        return Ok(vec![
            Anchor::new(start, [0.0; 3], h, AnchorKind::Smooth),
            Anchor::new(
                add(start, scale(t, length)),
                scale(h, -1.0),
                [0.0; 3],
                AnchorKind::Smooth,
            ),
        ]);
    }
    let n = normalize(n);
    let (theta, r) = (b.angle, b.radius);
    let arc = r * theta;
    let lead = (b.pivot * length - arc / 2.0).clamp(0.0, length - arc);
    let tail = length - lead - arc;
    let end_dir = add(scale(t, theta.cos()), scale(n, theta.sin()));

    let p1 = add(start, scale(t, lead));
    let mut anchors = vec![Anchor::new(start, [0.0; 3], [0.0; 3], AnchorKind::Smooth)];
    if lead > 1e-9 {
        anchors[0].handle_out = scale(t, lead / 3.0);
        anchors.push(Anchor::new(
            p1,
            scale(t, -lead / 3.0),
            [0.0; 3],
            AnchorKind::Smooth,
        ));
    }
    let p2 = if arc > 1e-9 {
        // Bue i planet utspent av T og N, med sentrum i p1 + r·N.
        let center = add(p1, scale(n, r));
        let pieces = (theta / MAX_PIECE).ceil() as usize;
        let step = theta / pieces as f64;
        let h = 4.0 / 3.0 * (step / 4.0).tan() * r;
        let at = |phi: f64| {
            let p = add(
                center,
                add(scale(n, -r * phi.cos()), scale(t, r * phi.sin())),
            );
            let d = add(scale(t, phi.cos()), scale(n, phi.sin()));
            (p, d)
        };
        anchors.last_mut().unwrap().handle_out = scale(t, h);
        for i in 1..=pieces {
            let (p, d) = at(step * i as f64);
            let out = if i < pieces { scale(d, h) } else { [0.0; 3] };
            anchors.push(Anchor::new(p, scale(d, -h), out, AnchorKind::Smooth));
        }
        anchors.last().unwrap().position
    } else {
        // Radius null: en eksplisitt knekk i p1.
        anchors.last_mut().unwrap().kind = AnchorKind::Corner;
        p1
    };
    if tail > 1e-9 {
        anchors.last_mut().unwrap().handle_out = scale(end_dir, tail / 3.0);
        anchors.push(Anchor::new(
            add(p2, scale(end_dir, tail)),
            scale(end_dir, -tail / 3.0),
            [0.0; 3],
            AnchorKind::Smooth,
        ));
    }
    if anchors.len() < 2 {
        return Err("Bøyen ga ingen senterlinje".into());
    }
    Ok(anchors)
}

/// Legger inn et punkt i andelen `at` av buelengden og bøyer resten av
/// senterlinjen derfra med låst lengde. Delen før punktet er uendret, og
/// den samlede buelengden er den samme. Slik kan en bøy fortsette forbi
/// 180° med flere bøyer etter hverandre.
pub fn bend_from(spline: &WorldSpline, at: f64, bend: &Bend) -> Result<Vec<Anchor>, String> {
    let at = at.clamp(0.0, 1.0);
    let rest = spline.length * (1.0 - at);
    if rest < 1e-9 {
        return Err("Det er ingen lengde igjen å bøye etter punktet".into());
    }
    let frame = spline.frame_at(at);
    let mut anchors = spline.head_until(at);
    let tail = bent_anchors(frame.origin, frame.t, rest, bend)?;
    // Punktet deles av hode og hale: inn-håndtak fra hodet, ut-håndtak fra halen.
    let joint = anchors.last_mut().unwrap();
    joint.handle_out = tail[0].handle_out;
    joint.kind = if tail[0].kind == AnchorKind::Corner {
        AnchorKind::Corner
    } else {
        AnchorKind::Smooth
    };
    anchors.extend_from_slice(&tail[1..]);
    Ok(anchors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::length;
    use crate::skeleton::{bezier_tangent, segments, WorldSpline};

    fn spline(b: &Bend) -> WorldSpline {
        let anchors = bent_anchors([0.0; 3], [0.0, 0.0, 1.0], 80.0, b).unwrap();
        WorldSpline::new(anchors, [1.0, 0.0, 0.0]).unwrap()
    }

    fn bend(deg: f64, radius: f64) -> Bend {
        Bend {
            direction: [1.0, 0.0, 0.0],
            angle: deg.to_radians(),
            radius,
            pivot: DEFAULT_PIVOT,
        }
    }

    fn end_tangent(w: &WorldSpline) -> Vec3 {
        let last = w.segments.last().unwrap();
        bezier_tangent(last, 1.0).unwrap()
    }

    #[test]
    fn buelengden_er_laast_ved_boying_radius_og_snapping() {
        let mut worst: f64 = 0.0;
        for deg in [0.0, 15.0, 45.0, 90.0, 135.0, 180.0] {
            for radius in [0.0, 5.0, 12.0, 25.0, 1000.0] {
                for pivot in [0.0, 0.3, 0.5, 1.0] {
                    let w = spline(&Bend {
                        pivot,
                        ..bend(deg, radius)
                    });
                    worst = worst.max((w.length - 80.0).abs() / 80.0);
                }
            }
        }
        // Snappet vinkel fra en vilkårlig musposisjon.
        let snapped = snap_angle(97.3_f64.to_radians(), 15.0_f64.to_radians());
        assert!((snapped.to_degrees() - 90.0).abs() < 1e-9);
        let w = spline(&Bend {
            angle: snapped,
            ..bend(0.0, 20.0)
        });
        worst = worst.max((w.length - 80.0).abs() / 80.0);
        eprintln!("største relative avvik i buelengde: {worst:e}");
        assert!(worst < LENGTH_TOLERANCE, "største relative avvik {worst:e}");
    }

    #[test]
    fn nitti_og_hundreaatti_grader_gir_riktig_enderetning() {
        let w = spline(&bend(90.0, 20.0));
        assert!(length(sub(end_tangent(&w), [1.0, 0.0, 0.0])) < 1e-9);
        // 180° vender enden tilbake som en U-bøy.
        let w = spline(&bend(180.0, 15.0));
        assert!(length(sub(end_tangent(&w), [0.0, 0.0, -1.0])) < 1e-9);
        let end = w.point_at(1.0);
        assert!((end[0] - 30.0).abs() < 1e-9, "{end:?}");
    }

    #[test]
    fn alle_retninger_paa_kulen_kan_naas() {
        for (dir, deg) in [
            ([0.0, 1.0, 0.0], 60.0),
            ([-1.0, -1.0, 0.0], 150.0),
            ([0.3, -0.8, 0.0], 120.0),
        ] {
            let b = Bend {
                direction: dir,
                ..bend(deg, 10.0)
            };
            let w = spline(&b);
            let d = normalize(dir);
            let expected = add(
                scale([0.0, 0.0, 1.0], deg.to_radians().cos()),
                scale(d, deg.to_radians().sin()),
            );
            assert!(length(sub(end_tangent(&w), expected)) < 1e-9);
        }
    }

    #[test]
    fn for_stor_radius_begrenses_uten_aa_forlenge() {
        // 180° med radius 100 får ikke plass i 80 mm; radius blir 80/π.
        let b = effective(&bend(180.0, 100.0), 80.0);
        assert!((b.radius - 80.0 / PI).abs() < 1e-12);
        let w = spline(&bend(180.0, 100.0));
        assert!((w.length - 80.0).abs() / 80.0 < LENGTH_TOLERANCE);
        // Hele lengden er bue: avstanden mellom endene er diameteren.
        assert!((length(w.point_at(1.0)) - 2.0 * 80.0 / PI).abs() < 1e-6);
    }

    #[test]
    fn radius_null_gir_en_gyldig_knekk() {
        let anchors = bent_anchors([0.0; 3], [0.0, 0.0, 1.0], 80.0, &bend(90.0, 0.0)).unwrap();
        assert_eq!(anchors.len(), 3);
        assert_eq!(anchors[1].kind, AnchorKind::Corner);
        assert!(length(sub(anchors[1].position, [0.0, 0.0, 40.0])) < 1e-12);
        let w = WorldSpline::new(anchors.clone(), [1.0, 0.0, 0.0]).unwrap();
        assert!((w.length - 80.0).abs() < 1e-9);
        assert!(segments(&anchors)
            .iter()
            .all(|s| s.iter().all(|p| p.iter().all(|x| x.is_finite()))));
        // Også en knekk på 180° og en knekk helt i starten er gyldige.
        for (deg, pivot) in [(180.0, 0.5), (90.0, 0.0), (90.0, 1.0)] {
            let a = bent_anchors(
                [0.0; 3],
                [0.0, 0.0, 1.0],
                80.0,
                &Bend {
                    pivot,
                    ..bend(deg, 0.0)
                },
            )
            .unwrap();
            let w = WorldSpline::new(a, [1.0, 0.0, 0.0]).unwrap();
            assert!((w.length - 80.0).abs() < 1e-9);
        }
    }

    #[test]
    fn rett_uttrekk_naar_vinkelen_er_null() {
        let w = spline(&bend(0.0, 20.0));
        assert!(length(sub(w.point_at(1.0), [0.0, 0.0, 80.0])) < 1e-9);
    }

    #[test]
    fn nytt_punkt_i_boyen_lar_den_fortsette_forbi_180_grader() {
        let first = spline(&Bend {
            pivot: 0.0,
            ..bend(180.0, 10.0)
        });
        // Buen er 10π mm lang og starter i roten; nytt punkt der den slutter.
        let at = 10.0 * PI / 80.0;
        let before = first.point_at(at * 0.5);
        let anchors = bend_from(
            &first,
            at,
            &Bend {
                pivot: 0.0,
                ..bend(180.0, 10.0)
            },
        )
        .unwrap();
        let w = WorldSpline::new(anchors, [1.0, 0.0, 0.0]).unwrap();
        // Lengden er låst, delen før punktet er uendret, og to 180° gir 360°.
        assert!(
            (w.length - 80.0).abs() / 80.0 < LENGTH_TOLERANCE,
            "{}",
            w.length
        );
        let moved = length(sub(w.point_at(at * 0.5), before));
        assert!(moved < LENGTH_TOLERANCE * 80.0, "{moved:e}");
        assert!(length(sub(end_tangent(&w), [0.0, 0.0, 1.0])) < 1e-6);
        // Over 180° i én bøy begrenses vinkelen.
        assert!((effective(&bend(270.0, 10.0), 80.0).angle - PI).abs() < 1e-12);
    }

    #[test]
    fn deling_av_kurven_endrer_ikke_formen() {
        let w = spline(&bend(90.0, 20.0));
        let head = WorldSpline::new(w.head_until(0.6), [1.0, 0.0, 0.0]).unwrap();
        assert!((head.length - 0.6 * w.length).abs() < 1e-6);
        for i in 0..=10 {
            let u = i as f64 / 10.0;
            assert!(length(sub(head.point_at(u), w.point_at(0.6 * u))) < 1e-6);
        }
    }

    #[test]
    fn boyen_starter_paa_midten_og_scroll_utvider_den_likt_til_begge_sider() {
        let mid = [45f64.to_radians().sin(), 0.0, 45f64.to_radians().cos()];
        let mut previous_lead = f64::INFINITY;
        for radius in [0.0, 5.0, 10.0, 20.0, 40.0] {
            let b = Bend::at_middle([1.0, 0.0, 0.0], PI / 2.0, radius);
            let anchors = bent_anchors([0.0; 3], [0.0, 0.0, 1.0], 80.0, &b).unwrap();
            let w = WorldSpline::new(anchors.clone(), [1.0, 0.0, 0.0]).unwrap();
            if radius > 0.0 {
                // Midt på kurven er buen halvveis: tangenten står på 45°.
                assert!(length(sub(w.frame_at(0.5).t, mid)) < 1e-6, "r = {radius}");
            }
            // Buen starter i 40 − rθ/2 mm og vokser like mye mot begge ender.
            let lead = anchors[1].position[2];
            let expected = 40.0 - radius * PI / 4.0;
            assert!((lead - expected).abs() < 1e-9, "r = {radius}: {lead}");
            assert!(lead <= previous_lead);
            previous_lead = lead;
            assert!((w.length - 80.0).abs() / 80.0 < LENGTH_TOLERANCE);
        }
    }
}
