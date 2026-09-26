/**
 * @format
 */

// Side-effect first: registers storage, the system colour scheme and the i18n
// host behind @voya/client and @voya/i18n, before any store module loads.
import './src/native/platform-boot';

// Uniwind compiles this sheet at build time and registers every class the app
// uses; importing it is what makes `className` mean anything.
import './global.css';

import { AppRegistry } from 'react-native';

import { App } from './src/app/App';
import { registerMobileBackend } from './src/ipc/platform';
import { name as appName } from './app.json';

// The native Rust host; tests register the shared mock instead.
registerMobileBackend();

AppRegistry.registerComponent(appName, () => App);
