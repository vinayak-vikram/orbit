use crossterm::event::{Event as Term, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures::StreamExt;
use orbit_client::{Client, Event};
use orbit_proto::consts::{COAP_PORT, MAX_MSG_SIZE};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, List, ListItem, Paragraph};

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let relay = args
        .next()
        .unwrap_or_else(|| format!("127.0.0.1:{COAP_PORT}"));
    let chan = args.next().unwrap_or_else(|| "general".into());
    let nick = args.next().unwrap_or_else(|| "anon".into());

    let (client, mut rx) = match Client::join(&relay, &chan, &nick).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("couldnt join {chan} on {relay}: {e:?}");
            std::process::exit(1);
        }
    };

    let mut term = ratatui::init();
    let mut keys = EventStream::new();
    let mut log: Vec<(String, String)> = vec![];
    let mut input = String::new();
    let title = format!(" {chan} on {relay} ");

    loop {
        let _ = term.draw(|f| {
            let [top, bottom] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).areas(f.area());
            // only the tail fits, and List truncates instead of wrapping. good enough for now
            let room = top.height.saturating_sub(2) as usize;
            let items: Vec<ListItem> = log
                .iter()
                .rev()
                .take(room)
                .rev()
                .map(|(who, what)| {
                    ListItem::new(format!("{who} {what}")).style(match who.as_str() {
                        "--" => Style::default().fg(Color::DarkGray),
                        _ => Style::default(),
                    })
                })
                .collect();
            f.render_widget(
                List::new(items).block(Block::bordered().title(title.clone())),
                top,
            );
            f.render_widget(
                Paragraph::new(input.as_str())
                    .block(Block::bordered().title(format!(" {} ", client.nick()))),
                bottom,
            );
            f.set_cursor_position((bottom.x + input.chars().count() as u16 + 1, bottom.y + 1));
        });

        tokio::select! {
            Some(Ok(Term::Key(k))) = keys.next() => {
                if k.kind != KeyEventKind::Press { continue }
                match k.code {
                    KeyCode::Esc => break,
                    KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Backspace => { input.pop(); }
                    // srv sends msg back to us bc its weird so we make sure to not show it
                    KeyCode::Enter if !input.trim().is_empty() => {
                        if let Err(e) = client.send(&input).await {
                            log.push(("--".into(), format!("not sent: {e:?}")));
                        }
                        input.clear();
                    }
                    KeyCode::Char(c) if input.len() < MAX_MSG_SIZE => input.push(c),
                    _ => {}
                }
            }
            Some(ev) = rx.recv() => match ev {
                Event::Line(l) => log.push((format!("<{}>", l.nick), l.text)),
                Event::Lost(why) => log.push(("--".into(), format!("relay: {why}"))),
            },
        }
    }

    ratatui::restore();
    // say goodbye, then give the deregister a beat to actually go out. leave() only pokes a
    // oneshot, the observe task still has to wake up and send it before the runtime dies
    let _ = client.send("left").await;
    client.leave();
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
}
