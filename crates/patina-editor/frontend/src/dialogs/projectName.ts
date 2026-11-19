// The name rule of a new project (15.10). The editor checks it again before it makes the folder.

/** The problem of a project name, or null when the name is valid or empty. */
export function projectNameProblem(name: string): string | null {
  if (name === '') return null;
  const ok = name.length <= 64 && !name.startsWith('.') && !/[. ]$/.test(name) && /^[A-Za-z0-9 _.-]+$/.test(name);
  return ok ? null : "Use letters, digits, spaces, '-', '_' and '.' in the name. Do not start or end it with '.'.";
}

/** The folder that New project makes: the location and the name, with the separator of the location. */
export function projectFolder(parent: string, name: string): string {
  const sep = parent.includes('\\') ? '\\' : '/';
  return parent.endsWith(sep) ? parent + name : `${parent}${sep}${name}`;
}
