/**
 * @format
 */

// Side-effect first: registers storage, the system colour scheme and the i18n
// host behind @voya/client and @voya/i18n, before any store module loads.
import './src/native/platform-boot';

import { AppRegistry } from 'react-native';

import { App } from './src/app/App';
import { name as appName } from './app.json';

AppRegistry.registerComponent(appName, () => App);
