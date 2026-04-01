#[cfg(test)]
pub mod test_utils {
    use ratatui::buffer::Buffer;

    pub fn buffer_to_string(buf: &Buffer) -> String {
        let mut output = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let cell = &buf[(x, y)];
                output.push_str(cell.symbol());
            }
            output.push('\n');
        }
        output
    }

    pub fn make_test_event(json: &str) -> crate::event::HookEvent {
        serde_json::from_str(json).unwrap()
    }
}
