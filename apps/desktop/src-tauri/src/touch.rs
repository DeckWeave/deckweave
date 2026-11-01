//! Single-contact probe adapter. No invented dial release, hold action or swipe navigation.
use deckweave_core::{Control, ControlKind, Geometry, InputEvent};
use deckweave_n4::Input;
use std::time::{Duration, Instant};

struct Contact {
    control: String,
    x: u16,
    y: Option<u16>,
    since: Instant,
}
#[derive(Default)]
pub struct TouchRouter {
    contact: Option<Contact>,
}
impl TouchRouter {
    pub fn route(
        &mut self,
        input: &Input,
        geometry: &Geometry,
        now: Instant,
    ) -> Option<(String, InputEvent)> {
        match *input {
            Input::Touch { x, pressed: true } => {
                if self.contact.is_some() {
                    return None;
                }
                let control = hit(geometry, x)?;
                self.contact = Some(Contact {
                    control: control.clone(),
                    x,
                    y: None,
                    since: now,
                });
                Some((control, InputEvent::Down))
            }
            Input::Touch { x, pressed: false } => self.contact.take().map(|c| {
                let event = if x < 800 && c.x.abs_diff(x) <= 30 {
                    InputEvent::Up
                } else {
                    InputEvent::Cancel
                };
                (c.control, event)
            }),
            Input::Pointer { x, y } => {
                let contact = self.contact.as_mut()?;
                let initial_y = *contact.y.get_or_insert(y);
                if x >= 800 || y >= 128 || contact.x.abs_diff(x) > 30 || initial_y.abs_diff(y) > 30
                {
                    return self.cancel();
                }
                None
            }
            Input::Swipe { .. } => self.cancel(),
            _ => None,
        }
    }
    pub fn expire(&mut self, now: Instant) -> Option<(String, InputEvent)> {
        if self
            .contact
            .as_ref()
            .is_some_and(|c| now.duration_since(c.since) >= Duration::from_secs(3))
        {
            self.cancel()
        } else {
            None
        }
    }
    fn cancel(&mut self) -> Option<(String, InputEvent)> {
        self.contact.take().map(|c| (c.control, InputEvent::Cancel))
    }
}

fn hit(geometry: &Geometry, x: u16) -> Option<String> {
    if x >= 800 {
        return None;
    }
    geometry
        .controls
        .iter()
        .find(|c| {
            let [left, top, width, height] = rect(geometry, c);
            c.kind == ControlKind::Touch
                && x >= left
                && x < left + width
                && 64 >= top
                && 64 < top + height
        })
        .map(|c| c.id.clone())
}

pub fn rect(geometry: &Geometry, control: &Control) -> [u16; 4] {
    let [left, top, width, height] = geometry.surface;
    [
        ((control.rect[0] - left) / width * 800.0).round() as u16,
        ((control.rect[1] - top) / height * 128.0).round() as u16,
        (control.rect[2] / width * 800.0).round() as u16,
        (control.rect[3] / height * 128.0).round() as u16,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use deckweave_core::geometry_for;
    #[test]
    fn actual_down_and_firmware_release_use_layout_and_original_contact() {
        let mut router = TouchRouter::default();
        let geometry = geometry_for("deckweave.n4");
        let now = Instant::now();
        let down = Input::Touch {
            x: 101,
            pressed: true,
        };
        assert!(
            matches!(router.route(&down, &geometry, now), Some((id, InputEvent::Down)) if id=="touch-1")
        );
        assert!(router.route(&down, &geometry, now).is_none());
        assert!(
            matches!(router.route(&Input::Touch { x: 101, pressed: false }, &geometry, now), Some((id, InputEvent::Up)) if id=="touch-1")
        );
        assert!(
            router
                .route(
                    &Input::Touch {
                        x: 101,
                        pressed: false
                    },
                    &geometry,
                    now
                )
                .is_none()
        );
        assert!(
            matches!(router.route(&Input::Touch { x: 723, pressed: true }, &geometry, now), Some((id, InputEvent::Down)) if id=="touch-5")
        );
    }
    #[test]
    fn movement_swipe_timeout_and_unknown_contact_never_execute_tap() {
        let geometry = geometry_for("deckweave.n4");
        let now = Instant::now();
        for finish in [
            Input::Pointer { x: 200, y: 60 },
            Input::Swipe { right: true },
            Input::Touch {
                x: 799,
                pressed: false,
            },
        ] {
            let mut router = TouchRouter::default();
            router.route(
                &Input::Touch {
                    x: 60,
                    pressed: true,
                },
                &geometry,
                now,
            );
            assert!(matches!(
                router.route(&finish, &geometry, now),
                Some((_, InputEvent::Cancel))
            ));
            assert!(
                router
                    .route(
                        &Input::Touch {
                            x: 60,
                            pressed: false
                        },
                        &geometry,
                        now
                    )
                    .is_none()
            );
        }
        let mut router = TouchRouter::default();
        assert!(
            router
                .route(&Input::Pointer { x: 60, y: 60 }, &geometry, now)
                .is_none()
        );
        router.route(
            &Input::Touch {
                x: 60,
                pressed: true,
            },
            &geometry,
            now,
        );
        assert!(matches!(
            router.expire(now + Duration::from_secs(3)),
            Some((_, InputEvent::Cancel))
        ));
        assert!(
            router
                .route(
                    &Input::Touch {
                        x: 900,
                        pressed: true
                    },
                    &geometry,
                    now
                )
                .is_none()
        );
    }
}
