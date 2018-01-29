#version 150 core

in vec2 a_Pos;
in vec4 a_Color;
out vec4 v_Color;

uniform vec2 i_Screen;

void main() {
    vec2 pos = (2.0 * a_Pos - i_Screen) / i_Screen;
    v_Color = a_Color;
    gl_Position = vec4(pos, 0.0, 1.0);
}
