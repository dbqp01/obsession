import { homedir } from 'node:os';
import { join } from 'node:path';

// Same place AnyWeb and Obsession's Connections panel look for the signed-in Chrome profile.
export const temuProfileDir = (env = process.env) => join(env.WEB2API_HOME || join(homedir(), '.web2api'), 'profiles', 'temu-chrome');
