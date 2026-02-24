use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use std::collections::HashSet;

use crate::app::{App, AppMode};
use crate::drums::DrumKind;
use crate::synth::note_name;

// ── Top-level routing ─────────────────────────────────────────────────────────

/// Draw all three panels simultaneously.  `app.mode` controls which panel has
/// keyboard focus (highlighted border), not what is visible.
pub fn draw(f: &mut Frame, app: &App, enhanced: bool) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // title bar
            Constraint::Length(12), // piano keyboard
            Constraint::Length(8),  // melodic step sequencer
            Constraint::Length(12), // drum machine
            Constraint::Length(4),  // status
            Constraint::Min(0),     // help
        ])
        .split(area);

    draw_title(f, chunks[0], enhanced, app);
    draw_piano(f, chunks[1], app);
    draw_synth_seq(f, chunks[2], app);
    draw_drums(f, chunks[3], app);
    draw_status(f, chunks[4], app);
    draw_help(f, chunks[5], app);
}

// ── Title bar ─────────────────────────────────────────────────────────────────

fn draw_title(f: &mut Frame, area: Rect, enhanced: bool, app: &App) {
    let focus_label = match app.mode {
        AppMode::Play     => "Keyboard",
        AppMode::SynthSeq => "Synth Seq",
        AppMode::Drums    => "Drums",
    };
    let kb_mode  = if enhanced { "enhanced" } else { "fallback" };
    let seq_ind  = if app.seq_playing()  { "  ▶SEQ"  } else { "" };
    let drum_ind = if app.drum_playing() { "  ▶DRUM" } else { "" };

    let text = format!(
        "  RustTuiSynth  ─  Focus: {}{}{}  ─  [{}]  ─  Tab/F2: cycle focus  F1: wave  F3: drums",
        focus_label, seq_ind, drum_ind, kb_mode
    );
    let color = if enhanced { Color::Cyan } else { Color::Yellow };
    f.render_widget(
        Paragraph::new(text)
            .style(Style::default().fg(color).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL)),
        area,
    );
}

// ── Piano keyboard ────────────────────────────────────────────────────────────

fn draw_piano(f: &mut Frame, area: Rect, app: &App) {
    let focused = app.mode == AppMode::Play;
    let title = if focused {
        " ► Keyboard — [←→] Octave  [↑↓] Volume  [Z-M / Q-P] Play notes "
    } else {
        " Keyboard "
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        });
    let inner = block.inner(area);
    f.render_widget(block, area);
    render_piano_widget(f, inner, &app.highlighted_notes());
}

fn render_piano_widget(f: &mut Frame, area: Rect, active: &HashSet<u8>) {
    let white_sem = [0u8, 2, 4, 5, 7, 9, 11];
    let has_black = [true, true, false, true, true, true, false];
    let black_sem = [1u8, 3, 0, 6, 8, 10, 0];
    let num_oct   = 2usize;
    let n_white   = white_sem.len() * num_oct + 1;

    let lower_white = ["z","x","c","v","b","n","m"];
    let upper_white = ["q","w","e","r","t","y","u"];
    let lower_black = ["s","d"," ","g","h","j"," "];
    let upper_black = ["2","3"," ","5","6","7"," "];
    let note_names  = ["C","D","E","F","G","A","B"];

    let mut lines: Vec<Line> = Vec::new();

    // Top border
    {
        let mut s = vec![Span::raw("┌")];
        for i in 0..n_white { s.push(Span::raw("───")); if i < n_white-1 { s.push(Span::raw("┬")); } }
        s.push(Span::raw("┐"));
        lines.push(Line::from(s));
    }

    // Black key rows
    for row in 0..4usize {
        let mut s = vec![Span::raw("│")];
        for wi in 0..n_white {
            let oct = wi / 7; let local_wi = wi % 7;
            let ws = if wi == n_white-1 { 0u8 } else { white_sem[local_wi] };
            let hb = wi < n_white-1 && has_black[local_wi];
            let bs = if hb { black_sem[local_wi] } else { 0 };
            let w_active = active.contains(&ws);

            let left_black = if local_wi > 0 { has_black[local_wi-1] } else { oct > 0 && has_black[6] };
            let lb_sem     = if local_wi > 0 && has_black[local_wi-1] { black_sem[local_wi-1] } else { 0 };
            let lb_active  = left_black && active.contains(&lb_sem);
            let rb_active  = hb && active.contains(&bs);

            let ws_style = if w_active { Style::default().bg(Color::Yellow).fg(Color::Black) }
                           else        { Style::default().bg(Color::White).fg(Color::Black) };
            let bk_active_sty = Style::default().bg(Color::Yellow).fg(Color::Black);
            let bk_sty        = Style::default().bg(Color::Black).fg(Color::White);

            let lc = if left_black { Span::styled("█", if lb_active { bk_active_sty } else { bk_sty }) }
                     else          { Span::styled(" ", ws_style) };
            let mc = if row == 3 {
                let label = if oct < num_oct { upper_black.get(local_wi).copied().unwrap_or(" ") } else { " " };
                Span::styled(label, ws_style)
            } else { Span::styled(" ", ws_style) };
            let rc = if hb { Span::styled("█", if rb_active { bk_active_sty } else { bk_sty }) }
                     else  { Span::styled(" ", ws_style) };
            s.push(lc); s.push(mc); s.push(rc); s.push(Span::raw("│"));
        }
        lines.push(Line::from(s));
    }

    // Black key label row
    {
        let mut s = vec![Span::raw("│")];
        for wi in 0..n_white {
            let oct = wi / 7; let local_wi = wi % 7;
            let ws = if wi == n_white-1 { 0u8 } else { white_sem[local_wi] };
            let hb = wi < n_white-1 && has_black[local_wi];
            let bs = if hb { black_sem[local_wi] } else { 0 };
            let w_active  = active.contains(&ws);
            let rb_active = hb && active.contains(&bs);

            let ll = if local_wi > 0 && has_black[local_wi-1] {
                if oct == 0 { lower_black[local_wi-1] } else { upper_black[local_wi-1] }
            } else { "" };
            let rl = if hb { if oct == 0 { lower_black[local_wi] } else { upper_black[local_wi] } } else { "" };

            let ws_sty   = if w_active { Style::default().bg(Color::Yellow).fg(Color::Black) }
                           else        { Style::default().bg(Color::White).fg(Color::Black) };
            let bk_a_sty = Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD);
            let bk_sty   = Style::default().bg(Color::Black).fg(Color::DarkGray);

            let lhb = local_wi > 0 && has_black[local_wi-1];
            let la  = lhb && active.contains(&black_sem[local_wi-1]);
            let lc  = if lhb { Span::styled(ll, if la { bk_a_sty } else { bk_sty }) } else { Span::styled(" ", ws_sty) };
            let mc  = Span::styled(" ", ws_sty);
            let rc  = if hb { Span::styled(rl, if rb_active { bk_a_sty } else { bk_sty }) } else { Span::styled(" ", ws_sty) };
            s.push(lc); s.push(mc); s.push(rc); s.push(Span::raw("│"));
        }
        lines.push(Line::from(s));
    }

    // Separator
    {
        let mut s = vec![Span::raw("│")];
        for wi in 0..n_white {
            let local_wi = wi % 7;
            let ws = if wi == n_white-1 { 0u8 } else { white_sem[local_wi] };
            let w_active = active.contains(&ws);
            let sty = if w_active { Style::default().bg(Color::Yellow).fg(Color::Black) }
                      else        { Style::default().bg(Color::White).fg(Color::Black) };
            let hbl = local_wi > 0 && has_black[local_wi-1];
            let hbr = wi < n_white-1 && has_black[local_wi];
            s.push(Span::styled(if hbl { "┘" } else { " " }, sty));
            s.push(Span::styled(" ", sty));
            s.push(Span::styled(if hbr { "└" } else { " " }, sty));
            s.push(Span::raw("│"));
        }
        lines.push(Line::from(s));
    }

    // White key labels
    {
        let mut s = vec![Span::raw("│")];
        for wi in 0..n_white {
            let oct = wi / 7; let local_wi = wi % 7;
            let ws = if wi == n_white-1 { 0u8 } else { white_sem[local_wi] };
            let w_active = active.contains(&ws);
            let sty = if w_active { Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD) }
                      else        { Style::default().bg(Color::White).fg(Color::DarkGray) };
            let label = if wi == n_white-1 { "" } else if oct == 0 { lower_white[local_wi] } else { upper_white[local_wi] };
            s.push(Span::styled(format!("{:^3}", label), sty));
            s.push(Span::raw("│"));
        }
        lines.push(Line::from(s));
    }

    // Note names
    {
        let mut s = vec![Span::raw("│")];
        for wi in 0..n_white {
            let local_wi = wi % 7;
            let ws = if wi == n_white-1 { 0u8 } else { white_sem[local_wi] };
            let w_active = active.contains(&ws);
            let sty = if w_active { Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD) }
                      else        { Style::default().bg(Color::White).fg(Color::Black) };
            let name = if wi == n_white-1 { "C" } else { note_names[local_wi] };
            s.push(Span::styled(format!("{:^3}", name), sty));
            s.push(Span::raw("│"));
        }
        lines.push(Line::from(s));
    }

    // Bottom border
    {
        let mut s = vec![Span::raw("└")];
        for i in 0..n_white { s.push(Span::raw("───")); if i < n_white-1 { s.push(Span::raw("┴")); } }
        s.push(Span::raw("┘"));
        lines.push(Line::from(s));
    }

    f.render_widget(Paragraph::new(lines), area);
}

// ── Melodic step sequencer ────────────────────────────────────────────────────

fn draw_synth_seq(f: &mut Frame, area: Rect, app: &App) {
    let focused = app.mode == AppMode::SynthSeq;
    let title = if focused {
        " ► Synth Seq — [←→] Cursor  [↑↓] BPM  [Space] Play  [Del] Clear  []] Steps "
    } else {
        " Synth Seq "
    };

    let (bpm, num_steps, current_step, playing, steps) = {
        let s = app.synth.lock().unwrap();
        (s.bpm, s.sequencer.num_steps, s.sequencer.current_step,
         s.sequencer.playing, s.sequencer.steps.clone())
    };
    let cursor = app.seq_cursor;
    let mut lines: Vec<Line> = Vec::new();

    // Header — BPM + status only (key hints are in the block title)
    let (status_str, status_color) =
        if playing { ("▶ PLAYING", Color::Green) } else { ("■ STOPPED", Color::DarkGray) };
    lines.push(Line::from(vec![
        Span::styled("BPM: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:.0}", bpm), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled("Steps: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{}", num_steps), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(status_str, Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(format!("Oct:{}", app.base_octave), Style::default().fg(Color::DarkGray)),
    ]));

    // Step rows — up to 16 per row
    let per_row = if num_steps <= 8 { 8 } else { 16 };
    for chunk_start in (0..num_steps).step_by(per_row) {
        let chunk_end = (chunk_start + per_row).min(num_steps);

        // Step number line
        let mut nums = Vec::new();
        for i in chunk_start..chunk_end {
            let is_ph = playing && i == current_step;
            let is_cu = i == cursor;
            let sty = if is_ph && is_cu { Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD) }
                      else if is_ph     { Style::default().fg(Color::Black).bg(Color::Green) }
                      else if is_cu     { Style::default().fg(Color::Black).bg(Color::Yellow) }
                      else              { Style::default().fg(Color::DarkGray) };
            nums.push(Span::styled(format!("{:^5}", i + 1), sty));
        }
        lines.push(Line::from(nums));

        // Step cell line — each cell 5 chars [xxx]
        let mut cells = Vec::new();
        for i in chunk_start..chunk_end {
            let is_ph = playing && i == current_step;
            let is_cu = i == cursor;
            let cell = match steps[i] {
                Some(n) => format!("[{:<3}]", note_name(n)),
                None    => "[ · ]".to_string(),
            };
            let sty = if is_ph && is_cu   { Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD) }
                      else if is_ph       { Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD) }
                      else if is_cu       { Style::default().fg(Color::Black).bg(Color::Yellow) }
                      else if steps[i].is_some() { Style::default().fg(Color::White) }
                      else               { Style::default().fg(Color::DarkGray) };
            cells.push(Span::styled(cell, sty));
        }
        lines.push(Line::from(cells));
    }

    // Cursor info line
    let note_disp = steps.get(cursor).copied().flatten()
        .map(|n| note_name(n)).unwrap_or_else(|| "·".to_string());
    lines.push(Line::from(vec![
        Span::styled("Cursor: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("step {}/{}  note: {}", cursor + 1, num_steps, note_disp),
            Style::default().fg(Color::White),
        ),
    ]));

    f.render_widget(
        Paragraph::new(lines).block(
            Block::default().title(title).borders(Borders::ALL)
                .border_style(if focused {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                })
        ),
        area,
    );
}

// ── Drum machine grid ─────────────────────────────────────────────────────────

fn drum_color(kind: DrumKind) -> Color {
    match kind {
        DrumKind::Kick      => Color::Red,
        DrumKind::Snare     => Color::Yellow,
        DrumKind::ClosedHat => Color::Cyan,
        DrumKind::OpenHat   => Color::Blue,
        DrumKind::Clap      => Color::Magenta,
        DrumKind::LowTom    => Color::Green,
        DrumKind::MidTom    => Color::LightGreen,
        DrumKind::HighTom   => Color::LightCyan,
    }
}

fn draw_drums(f: &mut Frame, area: Rect, app: &App) {
    let focused = app.mode == AppMode::Drums;
    let title = if focused {
        " ► Drum Machine — [↑↓] Track  [←→] Step  [Space] Toggle  [\\] Mute  [-=] Vol  []] Steps "
    } else {
        " Drum Machine "
    };

    // Snapshot drum state under the lock, then release before rendering
    let (bpm, num_steps, current_step, playing, tracks) = {
        let s = app.synth.lock().unwrap();
        let dm = &s.drum_machine;
        let tracks: Vec<(DrumKind, Vec<bool>, bool, f32)> =
            dm.tracks.iter().map(|t| (t.kind, t.steps.clone(), t.muted, t.volume)).collect();
        (s.bpm, dm.num_steps, dm.current_step, dm.playing, tracks)
    };
    let sel_track = app.drum_track;
    let sel_step  = app.drum_step;

    let mut lines: Vec<Line> = Vec::new();

    // ── Header ────────────────────────────────────────────────────────────
    let (status_str, status_color) =
        if playing { ("▶ PLAYING", Color::Green) } else { ("■ STOPPED", Color::DarkGray) };
    lines.push(Line::from(vec![
        Span::styled("BPM: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{:.0}", bpm), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled("Steps: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{}", num_steps), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(status_str, Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
    ]));

    // ── Step number header ────────────────────────────────────────────────
    // Label column: 14 chars to match track row label width
    {
        let mut s = vec![Span::styled("              ", Style::default())]; // 14 spaces
        for i in 0..num_steps {
            let is_ph = playing && i == current_step;
            let label = if i % 4 == 0 { format!("{:>2}", i + 1) } else { " .".to_string() };
            let sty = if is_ph { Style::default().fg(Color::Green).add_modifier(Modifier::BOLD) }
                      else     { Style::default().fg(Color::DarkGray) };
            s.push(Span::styled(label, sty));
        }
        lines.push(Line::from(s));
    }

    // ── Track rows ────────────────────────────────────────────────────────
    for (ti, (kind, steps, muted, volume)) in tracks.iter().enumerate() {
        let is_selected = ti == sel_track;
        let track_color = drum_color(*kind);
        let vol_pct = (volume * 100.0).round() as u32;

        // Mute indicator
        let mute_char  = if *muted { 'M' } else { '·' };
        let name_style = if is_selected && !muted {
            Style::default().fg(track_color).add_modifier(Modifier::BOLD)
        } else if is_selected {
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)
        } else if *muted {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(track_color)
        };
        let mute_style = if *muted {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let vol_style = if is_selected && focused {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        // Label: " Name [·]VVV%│" = 14 chars
        //  1 + 5 + 1 + 1 + 1 + 3 + 1 + 1 = 14
        let mut row: Vec<Span> = vec![
            Span::styled(format!(" {:5}", kind.name()), name_style),
            Span::styled("[", Style::default().fg(Color::DarkGray)),
            Span::styled(mute_char.to_string(), mute_style),
            Span::styled("]", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:3}%", vol_pct), vol_style),
            Span::styled("│", Style::default().fg(Color::DarkGray)),
        ];

        // Step cells — 2 chars each, beat separator every 4
        for i in 0..num_steps {
            let active  = steps.get(i).copied().unwrap_or(false);
            let is_ph   = playing && i == current_step;
            let is_cu   = is_selected && i == sel_step;

            let cell_char = if active { "█" } else { "·" };

            let sty = if is_ph && is_cu {
                Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else if is_ph {
                Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)
            } else if is_cu {
                Style::default().fg(Color::Black).bg(Color::Yellow)
            } else if active && !muted {
                Style::default().fg(track_color).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            if i > 0 && i % 4 == 0 {
                row.push(Span::styled("┆", Style::default().fg(Color::DarkGray)));
            }
            row.push(Span::styled(format!("{} ", cell_char), sty));
        }

        lines.push(Line::from(row));
    }

    f.render_widget(
        Paragraph::new(lines).block(
            Block::default().title(title).borders(Borders::ALL)
                .border_style(if focused {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                })
        ),
        area,
    );
}

// ── Status bar ────────────────────────────────────────────────────────────────

fn draw_status(f: &mut Frame, area: Rect, app: &App) {
    let wave    = app.wave_name();
    let vol     = app.volume();
    let bpm     = { app.synth.lock().unwrap().bpm };
    let notes   = app.active_note_names();
    let notes_s = if notes.is_empty() { "—".to_string() } else { notes.join(" ") };
    let extra   = if app.status_msg.is_empty() { String::new() } else { format!("  │  {}", app.status_msg) };

    let text = vec![
        Line::from(vec![
            Span::styled("Wave: ",   Style::default().fg(Color::DarkGray)),
            Span::styled(&wave,      Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("  │  "),
            Span::styled("BPM: ",    Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}", bpm), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("  │  "),
            Span::styled("Vol: ",    Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:.0}%", vol * 100.0),
                         Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::styled(&extra,     Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("Playing: ", Style::default().fg(Color::DarkGray)),
            Span::styled(notes_s,     Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
    ];

    f.render_widget(
        Paragraph::new(text)
            .block(Block::default().title(" Status ").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        area,
    );
}

// ── Unified help panel ────────────────────────────────────────────────────────

fn draw_help(f: &mut Frame, area: Rect, app: &App) {
    let w = Style::default().fg(Color::White);
    let d = Style::default().fg(Color::DarkGray);

    // Global line shown in all focus modes
    let global = Line::from(vec![
        Span::styled("[Tab/F2] ", w), Span::raw("Cycle focus  │  "),
        Span::styled("[F1] ",     w), Span::raw("Waveform  │  "),
        Span::styled("[F3] ",     w), Span::raw("Drum play/stop  │  "),
        Span::styled("[PgUp/Dn] ",w), Span::raw("BPM  │  "),
        Span::styled("[Esc] ",    w), Span::raw("Quit"),
    ]);

    // Focus-specific line
    let focus_line = match app.mode {
        AppMode::Play => Line::from(vec![
            Span::styled("Keys: ", d),
            Span::raw("Z X C V B N M  (white)  S D G H J  (black)  │  upper row: Q-P / 2-0"),
        ]),
        AppMode::SynthSeq => Line::from(vec![
            Span::styled("Piano keys: ", d),
            Span::raw("set note at cursor (advances)  │  "),
            Span::styled("[Space] ", w), Span::raw("Play/Pause  │  "),
            Span::styled("[Del] ",   w), Span::raw("Clear  │  "),
            Span::styled("[]] ",     w), Span::raw("Cycle steps"),
        ]),
        AppMode::Drums => Line::from(vec![
            Span::styled("Preview: ", d),
            Span::styled("Z",  Style::default().fg(Color::Red)),     Span::raw(" Kick  "),
            Span::styled("X",  Style::default().fg(Color::Yellow)),  Span::raw(" Snare  "),
            Span::styled("C",  Style::default().fg(Color::Cyan)),    Span::raw(" C-Hat  "),
            Span::styled("V",  Style::default().fg(Color::Blue)),    Span::raw(" O-Hat  "),
            Span::styled("B",  Style::default().fg(Color::Magenta)), Span::raw(" Clap  "),
            Span::styled("N",  Style::default().fg(Color::Green)),   Span::raw(" L.Tom  "),
            Span::styled("M",  Style::default().fg(Color::LightGreen)), Span::raw(" M.Tom  "),
            Span::styled(",",  Style::default().fg(Color::LightCyan)),  Span::raw(" H.Tom  │  "),
            Span::styled("[Enter] ", w), Span::raw("Play  │  "),
            Span::styled("[\\ ] ", w),  Span::raw("Mute  │  "),
            Span::styled("[Del] ",  w), Span::raw("Clear"),
        ]),
    };

    f.render_widget(
        Paragraph::new(vec![global, focus_line])
            .block(Block::default().title(" Help ").borders(Borders::ALL))
            .style(Style::default().fg(Color::DarkGray)),
        area,
    );
}
