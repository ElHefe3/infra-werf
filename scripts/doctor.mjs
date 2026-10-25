import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, mkdirSync, mkdtempSync, writeFileSync, symlinkSync, rmSync, rmdirSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const report = [];
function check(name, ready, detail, required = true) {
  report.push({ name, ready, required, detail });
}
function command(name, executable, args, accepts = () => true) {
  try { const result = execFileSync(executable, args, { encoding: 'utf8', timeout: 20000, windowsHide: true }).trim(); check(name, accepts(result), result); }
  catch (e) { check(name, false, e.message.split('\n')[0]); }
}
check('node', process.version === 'v26.12.1', `${process.version} (${process.execPath})`);
check('pnpm', process.env.ATLAS_PNPM_VERSION === '12.15.0', process.env.ATLAS_PNPM_VERSION || 'Run through mise run doctor');
command('rust', 'rustc', ['--version'], v => v.startsWith('rustc 1.99.1 '));
command('cargo', 'cargo', ['--version'], v => v.startsWith('cargo 1.99.1 '));
command('rust-host', 'rustup', ['show', 'active-toolchain'], v => v.startsWith('1.99.1-x86_64-pc-windows-msvc'));
command('android-rust-target', 'rustup', ['target', 'list', '--installed'], v => v.split(/\s+/).includes('aarch64-linux-android'));
command('python', 'python', ['--version'], v => v === 'Python 3.15.1');
command('java', join(process.env.JAVA_HOME || '', 'bin', 'java.exe'), ['--version'], v => v.includes('21.0.12'));
const sdk = process.env.ANDROID_HOME;
check('android-sdk', !!sdk && existsSync(sdk), sdk || 'ANDROID_HOME is missing');
const androidBuildFile = new URL('../src-tauri/gen/android/app/build.gradle.kts', import.meta.url);
const compileSdk = existsSync(androidBuildFile) ? readFileSync(androidBuildFile, 'utf8').match(/compileSdk\s*=\s*(\d+)/)?.[1] : undefined;
// New SDK packages use minor API directory names (for example android-37.0).
// Inspect package metadata instead of assuming android-<integer> naming.
const platformDirectory = sdk && join(sdk, 'platforms');
const matchingPlatforms = compileSdk && platformDirectory && existsSync(platformDirectory)
  ? readdirSync(platformDirectory).filter(name => {
      const directory = join(platformDirectory, name);
      const properties = join(directory, 'source.properties');
      if (!existsSync(properties) || !existsSync(join(directory, 'android.jar'))) return false;
      const api = readFileSync(properties, 'utf8').match(/^AndroidVersion.ApiLevel\s*=\s*(\d+(?:\.\d+)?)\s*$/m)?.[1];
      return api !== undefined && Number(api) === Number(compileSdk);
    }) : [];
check('android-platform', matchingPlatforms.length > 0, compileSdk ? `Requires API ${compileSdk}; matching installed packages: ${matchingPlatforms.join(', ') || 'none'}` : 'Run the Android project initialization first');
check('adb', !!sdk && existsSync(join(sdk, 'platform-tools', 'adb.exe')), 'Android platform tools');
const ndkProperties = process.env.NDK_HOME && join(process.env.NDK_HOME, 'source.properties');
const ndkVersion = ndkProperties && existsSync(ndkProperties) ? readFileSync(ndkProperties, 'utf8').match(/Pkg.Revision\s*=\s*(\S+)/)?.[1] : undefined;
check('android-ndk', ndkVersion === process.env.ATLAS_ANDROID_NDK_VERSION, `${process.env.NDK_HOME || 'NDK_HOME is missing'}; version=${ndkVersion || 'missing'}`);
// Test the permission Tauri actually uses, without changing Windows settings.
// All transient probe files stay in the project's ignored tools directory.
let probe;
try {
  const toolsDirectory = fileURLToPath(new URL('../.tools/', import.meta.url));
  mkdirSync(toolsDirectory, { recursive: true });
  probe = mkdtempSync(join(toolsDirectory, 'link-probe-'));
  writeFileSync(join(probe, 'source'), 'Atlas symbolic-link readiness probe');
  symlinkSync(join(probe, 'source'), join(probe, 'link'), 'file');
  check('android-symbolic-links', true, 'Native-library link creation succeeded in this process context');
} catch (e) {
  check('android-symbolic-links', false, `${e.code || 'error'}: native-library symbolic links need permission in the build context; see D008`);
} finally {
  if (probe) {
    rmSync(join(probe, 'link'), { force: true });
    rmSync(join(probe, 'source'), { force: true });
    rmdirSync(probe);
  }
}
check('sdk-command-tools', !!sdk && existsSync(join(sdk, 'cmdline-tools')), 'Optional until additional SDK packages are required', false);
const installers = ['C:/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe', 'C:/Program Files/Microsoft Visual Studio/Installer/vswhere.exe'];
const vswhere = installers.find(existsSync);
if (vswhere) {
  try {
    const locations = execFileSync(vswhere, ['-latest', '-products', '*', '-requires', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-property', 'installationPath'], { encoding: 'utf8', windowsHide: true }).trim();
    check('msvc', !!locations, locations || 'C++ x64 compiler workload not installed');
    const tools = locations && join(locations.split(/\r?\n/)[0], 'VC', 'Tools', 'MSVC');
    const linkers = tools && existsSync(tools) ? readdirSync(tools).map(v => join(tools, v, 'bin', 'Hostx64', 'x64', 'link.exe')).filter(existsSync) : [];
    check('msvc-linker', linkers.length > 0, linkers.join(', ') || 'link.exe not found');
  } catch (e) { check('msvc', false, e.message); }
} else {
  check('msvc', false, 'Visual Studio Installer / C++ workload not found');
  check('msvc-linker', false, 'No compiler installation available to locate link.exe');
}
const sdkLib = 'C:/Program Files (x86)/Windows Kits/10/Lib';
const sdkVersions = existsSync(sdkLib) ? readdirSync(sdkLib).filter(v => existsSync(join(sdkLib, v, 'um', 'x64', 'kernel32.lib')) && existsSync(join(sdkLib, v, 'ucrt', 'x64', 'ucrt.lib'))) : [];
check('windows-sdk', sdkVersions.length > 0, sdkVersions.join(', ') || 'Windows SDK x64 libraries are missing');
const webview = 'C:/Program Files (x86)/Microsoft/EdgeWebView/Application';
check('webview2', existsSync(webview) && readdirSync(webview).some(n => /^\d/.test(n)), 'Installed runtime directory; native launch proves actual usability');
check('android-signing', false, 'Release signing is not configured (T025)', false);
check('physical-phone', false, 'Actual-device acceptance remains T026', false);
check('ssh-fixture', false, 'Controlled SSH/sudo fixture not established yet (before F06)', false);
console.log(JSON.stringify({ platform: process.platform, checks: report }, null, 2));
if (report.some(r => r.required && !r.ready)) process.exitCode = 2;
