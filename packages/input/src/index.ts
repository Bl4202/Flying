/** While the tab is focused the input pump also samples on this timer, besides every frame (GDD 58.2). */
export const INPUT_POLL_TIMER_MS = 4;
/** An unchanged input state is still written at least this often, so the sim sees fresh `t_ms`. */
export const INPUT_MAX_SILENCE_MS = 8;
