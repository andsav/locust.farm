/** Small WebGL2 helpers. They throw on failure; the caller decides how to degrade. */

export interface Program {
	readonly handle: WebGLProgram;
	readonly uniforms: Readonly<Record<string, WebGLUniformLocation>>;
}

export interface ProgramSource {
	vertex: string;
	fragment: string;
	/** Vertex outputs captured by transform feedback, interleaved in this order. */
	feedback?: string[];
}

/**
 * Compiles and links a set of programs. Every compile and link is issued before
 * any status is read, so drivers that compile on other threads do so in parallel.
 */
export function createPrograms<Name extends string>(
	gl: WebGL2RenderingContext,
	sources: Record<Name, ProgramSource>
): Record<Name, Program> {
	const names = Object.keys(sources) as Name[];
	const handles = names.map((name) => {
		const { vertex, fragment, feedback } = sources[name];
		const handle = gl.createProgram();
		for (const [type, source] of [
			[gl.VERTEX_SHADER, vertex],
			[gl.FRAGMENT_SHADER, fragment]
		] as const) {
			const shader = gl.createShader(type)!;
			gl.shaderSource(shader, source);
			gl.compileShader(shader);
			gl.attachShader(handle, shader);
			// Flagged for deletion now, freed when the program is.
			gl.deleteShader(shader);
		}
		if (feedback) gl.transformFeedbackVaryings(handle, feedback, gl.INTERLEAVED_ATTRIBS);
		gl.linkProgram(handle);
		return handle;
	});

	const programs = {} as Record<Name, Program>;
	names.forEach((name, i) => {
		const handle = handles[i];
		if (!gl.getProgramParameter(handle, gl.LINK_STATUS) && !gl.isContextLost()) {
			const logs = [
				gl.getProgramInfoLog(handle),
				...(gl.getAttachedShaders(handle) ?? []).map((shader) => gl.getShaderInfoLog(shader))
			];
			throw new Error(
				`Swarm shader "${name}" failed to build:\n${logs.filter(Boolean).join('\n')}`
			);
		}
		const uniforms: Record<string, WebGLUniformLocation> = {};
		const count = gl.getProgramParameter(handle, gl.ACTIVE_UNIFORMS) as number;
		for (let u = 0; u < count; u++) {
			// Array uniforms are reported as "name[0]"; they are looked up by their plain name.
			const uniform = gl.getActiveUniform(handle, u)!.name.replace(/\[0\]$/, '');
			uniforms[uniform] = gl.getUniformLocation(handle, uniform)!;
		}
		programs[name] = { handle, uniforms };
	});
	return programs;
}

export interface Attribute {
	location: number;
	/** Number of float components. */
	size: number;
	/** Byte offset within one element of the buffer. */
	offset?: number;
	type?: GLenum;
}

/**
 * Points attributes of the bound vertex array at `buffer`. With `instanced`, each
 * attribute advances once per instance rather than once per vertex.
 */
export function bindAttributes(
	gl: WebGL2RenderingContext,
	buffer: WebGLBuffer,
	stride: number,
	attributes: Attribute[],
	instanced = false
): void {
	gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
	for (const { location, size, offset = 0, type = gl.FLOAT } of attributes) {
		gl.enableVertexAttribArray(location);
		gl.vertexAttribPointer(location, size, type, false, stride, offset);
		gl.vertexAttribDivisor(location, instanced ? 1 : 0);
	}
	gl.bindBuffer(gl.ARRAY_BUFFER, null);
}

/** A color target: a texture and the framebuffer that renders into it. */
export interface Target {
	readonly texture: WebGLTexture;
	readonly framebuffer: WebGLFramebuffer;
	readonly width: number;
	readonly height: number;
}

export function createTarget(gl: WebGL2RenderingContext, width: number, height: number): Target {
	const texture = gl.createTexture();
	gl.bindTexture(gl.TEXTURE_2D, texture);
	gl.texStorage2D(gl.TEXTURE_2D, 1, gl.RGBA8, width, height);
	gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
	gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
	gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
	gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
	gl.bindTexture(gl.TEXTURE_2D, null);

	const framebuffer = gl.createFramebuffer();
	gl.bindFramebuffer(gl.FRAMEBUFFER, framebuffer);
	gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, texture, 0);
	gl.bindFramebuffer(gl.FRAMEBUFFER, null);
	return { texture, framebuffer, width, height };
}

export function deleteTarget(gl: WebGL2RenderingContext, target: Target | undefined): void {
	if (!target) return;
	gl.deleteFramebuffer(target.framebuffer);
	gl.deleteTexture(target.texture);
}
