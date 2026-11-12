pub const WIDTH: usize = 10;
pub const HEIGHT: usize = 20;
pub const TOTAL_HEIGHT: usize = 22; // Includes two hidden rows

pub struct Board {
    pub cells: [[bool; WIDTH]; TOTAL_HEIGHT],
}

impl Board {
    pub fn new() -> Self {
        Self {
            cells: [[false; WIDTH]; TOTAL_HEIGHT],
        }
    }

    pub fn is_empty(&self, row: i32, col: i32) -> bool {
        if row < 0 || row >= TOTAL_HEIGHT as i32 {
            return false;
        }

        if col < 0 || col >= WIDTH as i32 {
            return false;
        }

        !self.cells[row as usize][col as usize]
    }

    pub fn can_place(&self, row: i32, col: i32, offsets: &[[i32; 2]; 4]) -> bool {
        for offset in offsets {
            let new_col = col + offset[0];
            let new_row = row + offset[1];

            if !self.is_empty(new_row, new_col) {
                return false;
            }
        }

        true
    }

    pub fn place_piece(&mut self, row: i32, col: i32, offsets: &[[i32; 2]; 4]) {
        for offset in offsets {
            let new_col = col + offset[0];
            let new_row = row + offset[1];

            if new_row >= 0
                && new_row < TOTAL_HEIGHT as i32
                && new_col >= 0
                && new_col < WIDTH as i32
            {
                self.cells[new_row as usize][new_col as usize] = true;
            }
        }
    }

    pub fn remove_full_rows(&mut self) -> usize {
        let mut cleared = 0;
        let mut row = 0;

        while row < HEIGHT {
            let mut full = true;

            for col in 0..WIDTH {
                if !self.cells[row][col] {
                    full = false;
                    break;
                }
            }

            if full {
                // shift all rows above the cleared row down
                for r in row..TOTAL_HEIGHT - 1 {
                    self.cells[r] = self.cells[r + 1];
                }

                self.cells[TOTAL_HEIGHT - 1] = [false; WIDTH];
                cleared += 1;
                // recheck this row after shifting
            } else {
                row += 1;
            }
        }

        cleared
    }

    pub fn render(&self, active_piece: Option<(i32, i32, &[[i32; 2]; 4])>) -> String {
        // draw the active piece without changing the board
        let mut display = self.cells;

        if let Some((row, col, offsets)) = active_piece {
            for offset in offsets {
                let new_col = col + offset[0];
                let new_row = row + offset[1];

                if new_row >= 0
                    && new_row < TOTAL_HEIGHT as i32
                    && new_col >= 0
                    && new_col < WIDTH as i32
                {
                    display[new_row as usize][new_col as usize] = true;
                }
            }
        }

        let mut output = String::new();

        // print from top to bottom
        for row in (0..TOTAL_HEIGHT).rev() {
            for col in 0..WIDTH {
                if display[row][col] {
                    output.push('o');
                } else {
                    output.push('.');
                }
            }

            output.push('\n');
        }

        output
    }
}
