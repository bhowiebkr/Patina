// The start screen steps of the self-check (check.ts): the settings screen, the About dialog and
// the content security policy (15.3, 15.10).

import { api } from '../api';
import { RUN_MS, button, type CheckContext } from './check-fixtures';

/** Runs the steps of the start screen before a project opens. */
export async function checkStart({ until, steps }: CheckContext) {
  // The settings screen (15.10): the start screen opens it. It shows the Rust version of the
  // toolchain and the list of trusted projects. Back returns to the start screen.
  const section = (name: string) => document.querySelector(`section[aria-label="${name}"]`)?.textContent ?? '';
  await until('the Settings button', RUN_MS, () => !!button('Settings'));
  button('Settings')!.click();
  const trustedList = () => section('Trusted projects').endsWith('None') || !!document.querySelector('.settings-row');
  await until('the toolchain and the trusted projects', RUN_MS, () => /Rust\d/.test(section('Toolchain')) && trustedList());
  // The check does not click Add from Git or Browse libraries: the downloads need the network (6.2).
  await until('the Git URL field of the libraries', RUN_MS, () => !!document.querySelector('input[aria-label="Git URL"]') && !!button('Add from Git') && !!button('Browse libraries'));
  await api.capture('settings');
  button('Back')!.click();
  // The check does not click Browse: the folder dialog of the platform would wait for a person.
  const browse = () => [...document.querySelectorAll('button')].filter((b) => b.textContent === 'Browse').length;
  await until('the start screen with two Browse buttons', RUN_MS, () => !!button('Settings') && browse() === 2);
  await api.capture('start');
  steps.push('settings screen');
  // The About dialog (15.3). The check does not click Check for updates: it needs the network.
  button('About')!.click();
  const about = () => document.querySelector('.dialog.about')?.textContent ?? '';
  await until('the About dialog with Check for updates', RUN_MS, () => !!button('Check for updates') && /Version \d/.test(about()));
  button('Close')!.click();
  await until('the start screen after About', RUN_MS, () => !button('Check for updates'));
  steps.push('about dialog');
  // The content security policy refuses code from text and requests to other hosts. The policy
  // refuses the request before a lookup, and a `.invalid` host never resolves.
  const refused = new Set<string>();
  const onRefuse = (e: SecurityPolicyViolationEvent) => refused.add(e.effectiveDirective);
  document.addEventListener('securitypolicyviolation', onRefuse);
  let evaluated = true;
  try {
    (0, eval)('1');
  } catch {
    evaluated = false;
  }
  const fetched = await fetch('https://patina.invalid/').then(() => true, () => false);
  await until('the reports of the content security policy', RUN_MS, () => refused.has('script-src') && refused.has('connect-src'));
  document.removeEventListener('securitypolicyviolation', onRefuse);
  if (evaluated || fetched) throw new Error(`the content security policy let ${evaluated ? 'eval' : 'a request to another host'} through`);
  steps.push('the content security policy refuses eval and other hosts');
}
