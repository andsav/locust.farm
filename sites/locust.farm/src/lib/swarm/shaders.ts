/**
 * GLSL ES 3.00 sources for the swarm renderer.
 *
 * Positions are in CSS pixels with the origin at the top left. `u_view` maps them
 * to clip space and `u_pixelRatio` converts them to device pixels for antialiasing.
 *
 * The draw shaders take `u_keep`, which blends their color toward the background.
 * Drawing with `u_keep` below 1 gives the color a full-strength draw would have
 * after the scene is faded by `1 - u_keep`, which lets one fade serve several steps.
 */

const glsl = String.raw;

/** Fragment shader for transform feedback passes, which rasterise nothing. */
export const DISCARD_FRAGMENT = glsl`#version 300 es
precision lowp float;
void main() {}
`;

/** Coverage of a one-pixel box filter centred at `x` over the interval [lo, hi], in device pixels. */
const COVERAGE = glsl`
float coverage(float x, float lo, float hi) {
	return clamp(x + 0.5, lo, hi) - clamp(x - 0.5, lo, hi);
}
`;

/**
 * Agent motion: attraction to the goal scaled by the agent's own speed, a constant
 * swirl around it, and jitter, followed by drag. `u_catchUp` adds pull in proportion
 * to distance, so agents left far behind hurry while those near the goal are unaffected.
 */
export const AGENT_UPDATE_VERTEX = glsl`#version 300 es
precision highp float;

layout(location = 0) in vec2 a_position;
layout(location = 1) in vec2 a_velocity;
layout(location = 2) in float a_speed;

uniform vec2 u_goal;
uniform float u_catchUp;
uniform uint u_step;

out vec2 v_position;
out vec2 v_velocity;

// PCG hash (Jarzynski and Olano, "Hash Functions for GPU Rendering").
uvec2 pcg2d(uvec2 v) {
	v = v * 1664525u + 1013904223u;
	v.x += v.y * 1664525u;
	v.y += v.x * 1664525u;
	v = v ^ (v >> 16u);
	v.x += v.y * 1664525u;
	v.y += v.x * 1664525u;
	v = v ^ (v >> 16u);
	return v;
}

void main() {
	vec2 toGoal = u_goal - a_position;
	float range = length(toGoal) + 1.0;
	vec2 pull = toGoal / range;
	float strength = 0.06 * a_speed * (1.0 + u_catchUp * range);
	vec2 jitter = vec2(pcg2d(uvec2(uint(gl_VertexID), u_step))) / 4294967296.0 - 0.5;

	vec2 velocity = a_velocity + pull * strength + vec2(-pull.y, pull.x) * 0.05 + jitter * 0.25;
	velocity *= 0.97;

	v_position = a_position + velocity;
	v_velocity = velocity;
}
`;

/**
 * Counts agents per density cell by drawing each as an additive point. Counts are
 * spread over the four 8-bit channels so a cell holds up to 1020 agents.
 */
export const DENSITY_VERTEX = glsl`#version 300 es
precision highp float;

layout(location = 0) in vec2 a_position;

uniform float u_cellSize;
uniform vec2 u_gridSize;

flat out vec4 v_count;

void main() {
	vec2 cell = floor(a_position / u_cellSize);
	v_count = vec4(equal(ivec4(gl_VertexID & 3), ivec4(0, 1, 2, 3))) / 255.0;
	// Agents outside the grid land outside clip space and are dropped.
	gl_Position = vec4((cell + 0.5) / u_gridSize * 2.0 - 1.0, 0.0, 1.0);
	gl_PointSize = 1.0;
}
`;

export const DENSITY_FRAGMENT = glsl`#version 300 es
precision mediump float;

flat in vec4 v_count;
out vec4 outColor;

void main() {
	outColor = v_count;
}
`;

/**
 * Locust cell motion: each cell is pushed away from nearby agents, using the
 * density grid, and pulled back to its place in the silhouette by a spring.
 */
export const CELL_UPDATE_VERTEX = glsl`#version 300 es
precision highp float;
precision highp sampler2D;

layout(location = 0) in vec2 a_position;
layout(location = 1) in vec2 a_velocity;
layout(location = 2) in float a_brightness;
layout(location = 3) in vec2 a_grid;
layout(location = 4) in float a_alive;

uniform sampler2D u_density;
uniform float u_cellSize;
uniform vec2 u_origin;
uniform float u_pitch;

out vec2 v_position;
out vec2 v_velocity;
out float v_brightness;

void main() {
	ivec2 gridSize = textureSize(u_density, 0);
	ivec2 center = ivec2(floor(a_position / u_cellSize));
	vec2 push = vec2(0.0);

	for (int j = -3; j <= 3; j++) {
		for (int i = -3; i <= 3; i++) {
			ivec2 cell = center + ivec2(i, j);
			if (any(lessThan(cell, ivec2(0))) || any(greaterThanEqual(cell, gridSize))) continue;
			float agents = dot(texelFetch(u_density, cell, 0), vec4(255.0));
			if (agents == 0.0) continue;
			vec2 away = a_position - (vec2(cell) + 0.5) * u_cellSize;
			float range = length(away) + 6.0;
			float falloff = range / 30.0;
			push += away / range * (0.009 * agents / (1.0 + falloff * falloff));
		}
	}
	float strength = length(push);
	if (strength > 0.6) push *= 0.6 / strength;

	vec2 home = u_origin + a_grid * u_pitch;
	vec2 velocity = (a_velocity + push + (home - a_position) * 0.012) * 0.88;

	v_position = a_position + velocity;
	v_velocity = velocity;
	v_brightness = a_brightness + (a_alive - a_brightness) * 0.06;
}
`;

/** Fills the target with one color. Drawn translucent, it fades the previous frame into trails. */
export const FILL_VERTEX = glsl`#version 300 es
void main() {
	// One triangle that covers the whole target.
	gl_Position = vec4(float(gl_VertexID % 2) * 4.0 - 1.0, float(gl_VertexID / 2) * 4.0 - 1.0, 0.0, 1.0);
}
`;

export const FILL_FRAGMENT = glsl`#version 300 es
precision mediump float;

uniform vec4 u_color;
out vec4 outColor;

void main() {
	outColor = u_color;
}
`;

/** One square per locust cell, antialiased so it moves smoothly between pixels. */
export const CELL_DRAW_VERTEX = glsl`#version 300 es
precision highp float;

layout(location = 0) in vec2 a_position;
layout(location = 2) in float a_brightness;
layout(location = 5) in float a_kind;

uniform vec2 u_view;
uniform float u_pixelRatio;
uniform float u_size;
uniform vec3 u_colors[3];
uniform vec3 u_background;
uniform float u_keep;

out vec2 v_pixel;
// Constant across a cell. Not declared flat, which is slow on some drivers for triangles.
out vec4 v_color;

void main() {
	vec2 corner = vec2(gl_VertexID & 1, gl_VertexID >> 1);
	float margin = 1.0 / u_pixelRatio;
	vec2 offset = mix(vec2(-margin), vec2(u_size + margin), corner);

	int kind = int(a_kind);
	// The eye is always opaque. Other cells brighten while alive.
	vec3 color = mix(u_background, u_colors[kind - 1], u_keep);
	v_color = vec4(color, kind == 3 ? 1.0 : 0.55 + 0.45 * a_brightness);
	v_pixel = offset * u_pixelRatio;
	gl_Position = vec4((a_position + offset) * u_view + vec2(-1.0, 1.0), 0.0, 1.0);
}
`;

export const CELL_DRAW_FRAGMENT = glsl`#version 300 es
precision highp float;

uniform float u_pixelRatio;
uniform float u_size;

in vec2 v_pixel;
in vec4 v_color;
out vec4 outColor;
${COVERAGE}
void main() {
	float size = u_size * u_pixelRatio;
	outColor = vec4(v_color.rgb, v_color.a * coverage(v_pixel.x, 0.0, size) * coverage(v_pixel.y, 0.0, size));
}
`;

/**
 * One streak per agent, from its position back along its velocity. Streaks are drawn
 * in two passes that share this vertex shader, so that a pixel under several streaks
 * is inked once, as when a single path is stroked.
 */
export const STREAK_VERTEX = glsl`#version 300 es
precision highp float;

layout(location = 0) in vec2 a_position;
layout(location = 1) in vec2 a_velocity;

uniform vec2 u_view;
uniform float u_pixelRatio;

out vec2 v_pixel;
out float v_length;

// Both passes must cover exactly the same pixels.
invariant gl_Position;

const float TAIL = 2.2;
const float WIDTH = 1.0;

void main() {
	vec2 corner = vec2(gl_VertexID & 1, gl_VertexID >> 1);
	vec2 tail = -a_velocity * TAIL;
	float len = length(tail);
	vec2 along = len > 0.0 ? tail / len : vec2(1.0, 0.0);
	vec2 across = vec2(-along.y, along.x);

	float margin = 1.0 / u_pixelRatio;
	vec2 local = mix(vec2(-margin, -0.5 * WIDTH - margin), vec2(len + margin, 0.5 * WIDTH + margin), corner);

	v_pixel = local * u_pixelRatio;
	v_length = len * u_pixelRatio;
	gl_Position = vec4((a_position + along * local.x + across * local.y) * u_view + vec2(-1.0, 1.0), 0.0, 1.0);
}
`;

/**
 * First pass: records in the alpha channel how much of each pixel shows through the
 * streaks. Blended with MIN, so overlapping streaks keep the strongest coverage.
 */
export const STREAK_COVERAGE_FRAGMENT = glsl`#version 300 es
precision highp float;

uniform float u_pixelRatio;
uniform float u_opacity;

in vec2 v_pixel;
in float v_length;
out vec4 outColor;
${COVERAGE}
void main() {
	float halfWidth = 0.5 * u_pixelRatio;
	float covered = coverage(v_pixel.x, 0.0, v_length) * coverage(v_pixel.y, -halfWidth, halfWidth);
	outColor = vec4(0.0, 0.0, 0.0, 1.0 - u_opacity * covered);
}
`;

/**
 * Second pass: inks each pixel by the coverage recorded for it and resets that
 * record, so further streaks over the same pixel add nothing.
 */
export const STREAK_INK_FRAGMENT = glsl`#version 300 es
precision mediump float;

uniform vec3 u_color;
uniform vec3 u_background;
uniform float u_keep;
out vec4 outColor;

void main() {
	outColor = vec4(mix(u_background, u_color, u_keep), 1.0);
}
`;

/** The goal: a filled circle. */
export const DOT_VERTEX = glsl`#version 300 es
precision highp float;

uniform vec2 u_view;
uniform float u_pixelRatio;
uniform vec2 u_center;
uniform float u_radius;

out vec2 v_pixel;

void main() {
	vec2 corner = vec2(gl_VertexID & 1, gl_VertexID >> 1);
	vec2 offset = (corner * 2.0 - 1.0) * (u_radius + 1.0 / u_pixelRatio);

	v_pixel = offset * u_pixelRatio;
	gl_Position = vec4((u_center + offset) * u_view + vec2(-1.0, 1.0), 0.0, 1.0);
}
`;

export const DOT_FRAGMENT = glsl`#version 300 es
precision highp float;

uniform float u_pixelRatio;
uniform float u_radius;
uniform vec3 u_color;
uniform vec3 u_background;
uniform float u_keep;

in vec2 v_pixel;
out vec4 outColor;

void main() {
	float covered = clamp(u_radius * u_pixelRatio + 0.5 - length(v_pixel), 0.0, 1.0);
	outColor = vec4(mix(u_background, u_color, u_keep), covered);
}
`;
