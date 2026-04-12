// Hello World — uses the Jack OS 8x11 font to render text on screen.
// draw_string(col, row, str): col 0-63, row 0-22

#define GREETING "Hello, World!"
#define COL 10
#define ROW 5

int main() {
    draw_string(COL, ROW, GREETING);
    return 0;
}
