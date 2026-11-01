//! Single-contact router. Unknown Y never selects a vertical region by guesswork.
use deckweave_core::{Control, ControlKind, Geometry, InputEvent, PointerPhase};
use deckweave_n4::Input;
use std::time::{Duration, Instant};
struct Contact {
    control: Option<String>,
    pointer: bool,
    x: u16,
    y: Option<u16>,
    last: (u16, Option<u16>),
    since: Instant,
}
#[derive(Default)]
pub struct TouchRouter {
    contact: Option<Contact>,
    sample: Option<(u16, u16, Instant)>,
}
fn event(phase: PointerPhase, x: u16, y: Option<u16>) -> InputEvent {
    InputEvent::Pointer {
        phase,
        x: f64::from(x.min(799)) / 800.0,
        y: y.map(|y| f64::from(y.min(127)) / 128.0),
    }
}
impl TouchRouter {
    pub fn route(
        &mut self,
        input: &Input,
        geometry: &Geometry,
        now: Instant,
    ) -> Option<(String, InputEvent)> {
        if self
            .contact
            .as_ref()
            .and_then(|c| c.control.as_ref())
            .is_some_and(|id| !geometry.controls.iter().any(|c| c.id == *id))
        {
            return self.cancel();
        }
        match *input {
            Input::Touch { x, pressed: true } => {
                if self.contact.is_some() || x >= 800 {
                    return None;
                }
                let y = self
                    .sample
                    .take()
                    .filter(|(px, _, at)| {
                        px.abs_diff(x) <= 12
                            && now.saturating_duration_since(*at) <= Duration::from_millis(50)
                    })
                    .map(|(_, y, _)| y);
                let target = hit(geometry, x, y);
                let result = target.map(|c| (c.id.clone(), event(PointerPhase::Down, x, y)));
                self.contact = Some(Contact {
                    control: target.map(|c| c.id.clone()),
                    pointer: target.is_some_and(|c| c.pointer),
                    x,
                    y,
                    last: (x, y),
                    since: now,
                });
                result
            }
            Input::Touch { x, pressed: false } => {
                self.sample = None;
                let c = self.contact.take()?;
                let id = c.control?;
                let phase = if x < 800 && (c.pointer || c.x.abs_diff(x) <= 30) {
                    PointerPhase::Up
                } else {
                    PointerPhase::Cancel
                };
                Some((id, event(phase, x, c.last.1)))
            }
            Input::Pointer { x, y } => {
                if x >= 800 || y >= 128 {
                    return self.cancel();
                }
                self.sample = Some((x, y, now));
                let c = self.contact.as_mut()?;
                let first_y = *c.y.get_or_insert(y);
                c.last = (x, Some(y));
                if c.control.is_none() {
                    if c.x.abs_diff(x) > 30 {
                        self.contact = None;
                        return None;
                    }
                    let target = hit(geometry, x, Some(y))?;
                    c.control = Some(target.id.clone());
                    c.pointer = target.pointer;
                    return Some((target.id.clone(), event(PointerPhase::Down, x, Some(y))));
                }
                if !c.pointer && (c.x.abs_diff(x) > 30 || first_y.abs_diff(y) > 30) {
                    return self.cancel();
                }
                Some((c.control.clone()?, event(PointerPhase::Move, x, Some(y))))
            }
            Input::Swipe { right } => {
                self.sample = None;
                let control = self
                    .contact
                    .as_ref()
                    .and_then(|c| c.control.clone())
                    .unwrap_or_else(|| "touch-strip".into());
                if !self.contact.as_ref().is_some_and(|c| c.pointer) {
                    self.contact = None;
                }
                Some((control, InputEvent::Swipe { right }))
            }
            _ => None,
        }
    }
    pub fn expire(&mut self, now: Instant) -> Option<(String, InputEvent)> {
        if self
            .contact
            .as_ref()
            .is_some_and(|c| now.saturating_duration_since(c.since) >= Duration::from_secs(3))
        {
            self.cancel()
        } else {
            None
        }
    }
    fn cancel(&mut self) -> Option<(String, InputEvent)> {
        self.sample = None;
        let c = self.contact.take()?;
        Some((c.control?, event(PointerPhase::Cancel, c.last.0, c.last.1)))
    }
}
fn hit(geometry: &Geometry, x: u16, y: Option<u16>) -> Option<&Control> {
    geometry.controls.iter().find(|c| {
        let [left, top, w, h] = rect(geometry, c);
        c.kind == ControlKind::Touch
            && x >= left
            && x < left + w
            && y.map_or(top == 0 && h == 128, |y| y >= top && y < top + h)
    })
}
pub fn rect(geometry: &Geometry, control: &Control) -> [u16; 4] {
    let [sx, sy, sw, sh] = geometry.surface;
    let x = ((control.rect[0] - sx) / sw * 800.0)
        .round()
        .clamp(0.0, 800.0) as u16;
    let y = ((control.rect[1] - sy) / sh * 128.0)
        .round()
        .clamp(0.0, 128.0) as u16;
    let right = ((control.rect[0] + control.rect[2] - sx) / sw * 800.0)
        .round()
        .clamp(f64::from(x), 800.0) as u16;
    let bottom = ((control.rect[1] + control.rect[3] - sy) / sh * 128.0)
        .round()
        .clamp(f64::from(y), 128.0) as u16;
    [x, y, right - x, bottom - y]
}
#[cfg(test)]
mod tests {
    use super::*;
    use deckweave_core::geometry_for;
    #[test]
    fn x_capture_preserves_unknown_y() {
        let mut r = TouchRouter::default();
        let g = geometry_for("deckweave.n4");
        let now = Instant::now();
        assert!(
            matches!(r.route(&Input::Touch{x:101,pressed:true},&g,now),Some((id,InputEvent::Pointer{phase:PointerPhase::Down,y:None,..})) if id=="touch-1")
        );
        assert!(
            r.route(
                &Input::Touch {
                    x: 101,
                    pressed: true
                },
                &g,
                now
            )
            .is_none()
        );
        assert!(
            matches!(r.route(&Input::Touch{x:101,pressed:false},&g,now),Some((id,InputEvent::Pointer{phase:PointerPhase::Up,..})) if id=="touch-1")
        );
        assert!(
            r.route(
                &Input::Touch {
                    x: 101,
                    pressed: false
                },
                &g,
                now
            )
            .is_none()
        );
    }
    #[test]
    fn vertical_regions_wait_for_y_and_native_drag_keeps_capture() {
        let mut r = TouchRouter::default();
        let mut g = geometry_for("deckweave.n4");
        let now = Instant::now();
        let c = g.controls.iter_mut().find(|c| c.id == "touch-1").unwrap();
        c.rect[3] /= 2.0;
        c.pointer = true;
        assert!(
            r.route(
                &Input::Touch {
                    x: 60,
                    pressed: true
                },
                &g,
                now
            )
            .is_none()
        );
        assert!(matches!(
            r.route(&Input::Pointer { x: 60, y: 20 }, &g, now),
            Some((
                _,
                InputEvent::Pointer {
                    phase: PointerPhase::Down,
                    ..
                }
            ))
        ));
        assert!(
            matches!(r.route(&Input::Pointer{x:600,y:100},&g,now),Some((id,InputEvent::Pointer{phase:PointerPhase::Move,..})) if id=="touch-1")
        );
        assert!(matches!(
            r.route(&Input::Swipe { right: true }, &g, now),
            Some((_, InputEvent::Swipe { .. }))
        ));
        assert!(matches!(
            r.route(
                &Input::Touch {
                    x: 600,
                    pressed: false
                },
                &g,
                now
            ),
            Some((
                _,
                InputEvent::Pointer {
                    phase: PointerPhase::Up,
                    ..
                }
            ))
        ));
    }
    #[test]
    fn motion_swipe_and_timeout_do_not_release_a_tap() {
        let g = geometry_for("deckweave.n4");
        let now = Instant::now();
        for input in [
            Input::Pointer { x: 200, y: 60 },
            Input::Touch {
                x: 799,
                pressed: false,
            },
        ] {
            let mut r = TouchRouter::default();
            r.route(
                &Input::Touch {
                    x: 60,
                    pressed: true,
                },
                &g,
                now,
            );
            assert!(matches!(
                r.route(&input, &g, now),
                Some((
                    _,
                    InputEvent::Pointer {
                        phase: PointerPhase::Cancel,
                        ..
                    }
                ))
            ));
            assert!(
                r.route(
                    &Input::Touch {
                        x: 60,
                        pressed: false
                    },
                    &g,
                    now
                )
                .is_none()
            );
        }
        let mut r = TouchRouter::default();
        r.route(
            &Input::Touch {
                x: 60,
                pressed: true,
            },
            &g,
            now,
        );
        assert!(matches!(
            r.route(&Input::Swipe { right: true }, &g, now),
            Some((_, InputEvent::Swipe { right: true }))
        ));
        assert!(
            r.route(
                &Input::Touch {
                    x: 60,
                    pressed: false
                },
                &g,
                now
            )
            .is_none()
        );
        r.route(
            &Input::Touch {
                x: 60,
                pressed: true,
            },
            &g,
            now,
        );
        assert!(matches!(
            r.expire(now + Duration::from_secs(3)),
            Some((
                _,
                InputEvent::Pointer {
                    phase: PointerPhase::Cancel,
                    ..
                }
            ))
        ));
    }
}
