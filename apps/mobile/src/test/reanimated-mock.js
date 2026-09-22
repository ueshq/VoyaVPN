/**
 * A standalone Reanimated mock.
 *
 * The shipped `react-native-reanimated/mock` still requires the real package,
 * and that package's import-time initializer calls `setCSSEventHandler` on the
 * JS fallback Jest gets without a native binary — which throws. HeroUI only
 * needs the public surface this stub covers; animations become identity maps.
 *
 * Missing names resolve to a chainable no-op constructor so a component that
 * reaches for the next layout preset does not explode the suite.
 */
/* eslint-disable @typescript-eslint/no-require-imports, no-undef */
const NOOP = () => {};
const ID = (value) => value;
const RN = require("react-native");

function sharedValue(init) {
  const box = { value: init };
  return new Proxy(box, {
    get(target, prop) {
      if (prop === "_isReanimatedSharedValue") return true;
      if (prop === "value") return target.value;
      if (prop === "get") return () => target.value;
      if (prop === "set") {
        return (next) => {
          target.value = typeof next === "function" ? next(target.value) : next;
        };
      }
      return Reflect.get(target, prop);
    },
    set(target, prop, next) {
      if (prop === "value") {
        target.value = next;
        return true;
      }
      return Reflect.set(target, prop, next);
    },
  });
}

function createAnimatedComponent(Component) {
  return Component;
}

class BaseAnimationMock {
  duration() {
    return this;
  }

  delay() {
    return this;
  }

  springify() {
    return this;
  }

  damping() {
    return this;
  }

  stiffness() {
    return this;
  }

  energyThreshold() {
    return this;
  }

  withCallback() {
    return this;
  }

  randomDelay() {
    return this;
  }

  withInitialValues() {
    return this;
  }

  easing() {
    return this;
  }

  rotate() {
    return this;
  }

  mass() {
    return this;
  }

  restDisplacementThreshold() {
    return this;
  }

  restSpeedThreshold() {
    return this;
  }

  overshootClamping() {
    return this;
  }

  dampingRatio() {
    return this;
  }

  getDelay() {
    return 0;
  }

  getDelayFunction() {
    return NOOP;
  }

  getDuration() {
    return 300;
  }

  getReduceMotion() {
    return 0;
  }

  getAnimationAndConfig() {
    return [NOOP, {}];
  }

  build() {
    return () => ({ initialValues: {}, animations: {} });
  }

  reduceMotion() {
    return this;
  }
}

const layoutPreset = () => new BaseAnimationMock();

const Animated = {
  View: RN.View,
  Text: RN.Text,
  Image: RN.Image,
  ScrollView: RN.ScrollView,
  FlatList: RN.FlatList,
  SectionList: RN.SectionList,
  Pressable: RN.Pressable,
  createAnimatedComponent,
};

const known = {
  __esModule: true,
  default: Animated,
  ...Animated,
  createAnimatedComponent,
  useSharedValue: sharedValue,
  useAnimatedStyle: (fn) => (typeof fn === "function" ? fn() : {}),
  useAnimatedProps: (fn) => (typeof fn === "function" ? fn() : {}),
  useDerivedValue: (fn) => sharedValue(typeof fn === "function" ? fn() : fn),
  useAnimatedReaction: NOOP,
  useAnimatedRef: () => ({ current: null }),
  useAnimatedScrollHandler: () => NOOP,
  useAnimatedSensor: () => ({
    sensor: { value: { x: 0, y: 0, z: 0, yaw: 0, pitch: 0, roll: 0 } },
    unregister: NOOP,
    isAvailable: false,
    config: { interval: 0, adjustToInterfaceOrientation: false, iosReferenceFrame: 0 },
  }),
  useReducedMotion: () => false,
  useFrameCallback: () => ({ isActive: false, setActive: NOOP, callbackId: -1 }),
  useComposedEventHandler: () => NOOP,
  useEvent: () => NOOP,
  useHandler: () => ({ context: {}, doDependenciesDiffer: false, useWeb: false }),
  useAnimatedKeyboard: () => ({ height: 0, state: 0 }),
  withTiming: (toValue, _config, callback) => {
    if (callback) callback(true);
    return toValue;
  },
  withSpring: (toValue, _config, callback) => {
    if (callback) callback(true);
    return toValue;
  },
  withDelay: (_delay, value) => value,
  withRepeat: (value) => value,
  withSequence: (...values) => values[values.length - 1],
  withDecay: (_config, callback) => {
    if (callback) callback(true);
    return 0;
  },
  cancelAnimation: NOOP,
  defineAnimation: ID,
  makeMutable: sharedValue,
  makeShareableCloneRecursive: ID,
  isSharedValue: (value) =>
    typeof value === "object" && value !== null && "value" in value,
  isWorkletFunction: () => false,
  getAnimatedStyle: () => ({}),
  getUseOfValueInStyleWarning: NOOP,
  getViewProp: () => Promise.resolve(undefined),
  processColor: RN.processColor,
  setUpTests: NOOP,
  enableLayoutAnimations: NOOP,
  configureReanimatedLogger: NOOP,
  createAnimatedPropAdapter: ID,
  createCSSAnimatedComponent: ID,
  Easing: {
    linear: ID,
    ease: ID,
    quad: ID,
    cubic: ID,
    poly: ID,
    sin: ID,
    circle: ID,
    exp: ID,
    elastic: ID,
    back: ID,
    bounce: ID,
    bezier: ID,
    in: ID,
    out: ID,
    inOut: ID,
    step0: ID,
    step1: ID,
  },
  Extrapolation: { EXTEND: "extend", CLAMP: "clamp", IDENTITY: "identity" },
  interpolate: (_value, _input, output) => (output ? output[0] : 0),
  runOnJS: (fn) => (...args) => fn(...args),
  runOnUI: (fn) => (...args) => fn(...args),
  Keyframe: class Keyframe extends BaseAnimationMock {
    constructor(definition) {
      super();
      this.definition = definition;
    }
  },
  BaseAnimationBuilder: BaseAnimationMock,
  ComplexAnimationBuilder: BaseAnimationMock,
  KeyboardState: { UNKNOWN: 0, OPENING: 1, OPEN: 2, CLOSING: 3, CLOSED: 4 },
  ReduceMotion: { System: 0, Always: 1, Never: 2 },
  SensorType: {
    ACCELEROMETER: 0,
    GYROSCOPE: 1,
    GRAVITY: 2,
    MAGNETIC_FIELD: 3,
    ROTATION: 4,
  },
  ReanimatedLogLevel: { warn: 1, error: 2, debug: 3 },
};

const PRESET_NAMES = [
  "FadeIn",
  "FadeInDown",
  "FadeInLeft",
  "FadeInRight",
  "FadeInUp",
  "FadeOut",
  "FadeOutDown",
  "FadeOutLeft",
  "FadeOutRight",
  "FadeOutUp",
  "FadingTransition",
  "FlipInEasyX",
  "FlipInEasyY",
  "FlipInXDown",
  "FlipInXUp",
  "FlipInYLeft",
  "FlipInYRight",
  "FlipOutEasyX",
  "FlipOutEasyY",
  "FlipOutXDown",
  "FlipOutXUp",
  "FlipOutYLeft",
  "FlipOutYRight",
  "BounceIn",
  "BounceInDown",
  "BounceInLeft",
  "BounceInRight",
  "BounceInUp",
  "BounceOut",
  "BounceOutDown",
  "BounceOutLeft",
  "BounceOutRight",
  "BounceOutUp",
  "LightSpeedInLeft",
  "LightSpeedInRight",
  "LightSpeedOutLeft",
  "LightSpeedOutRight",
  "PinwheelIn",
  "PinwheelOut",
  "RollInLeft",
  "RollInRight",
  "RollOutLeft",
  "RollOutRight",
  "RotateInDownLeft",
  "RotateInDownRight",
  "RotateInUpLeft",
  "RotateInUpRight",
  "RotateOutDownLeft",
  "RotateOutDownRight",
  "RotateOutUpLeft",
  "RotateOutUpRight",
  "SlideInDown",
  "SlideInLeft",
  "SlideInRight",
  "SlideInUp",
  "SlideOutDown",
  "SlideOutLeft",
  "SlideOutRight",
  "SlideOutUp",
  "StretchInX",
  "StretchInY",
  "StretchOutX",
  "StretchOutY",
  "ZoomIn",
  "ZoomInDown",
  "ZoomInEasyDown",
  "ZoomInEasyUp",
  "ZoomInLeft",
  "ZoomInRight",
  "ZoomInRotate",
  "ZoomInUp",
  "ZoomOut",
  "ZoomOutDown",
  "ZoomOutEasyDown",
  "ZoomOutEasyUp",
  "ZoomOutLeft",
  "ZoomOutRight",
  "ZoomOutRotate",
  "ZoomOutUp",
  "CurvedTransition",
  "EntryExitTransition",
  "FadingTransition",
  "JumpingTransition",
  "Layout",
  "LinearTransition",
  "SequencedTransition",
  "SharedTransition",
];

for (const name of PRESET_NAMES) {
  known[name] = layoutPreset();
}

// Anything HeroUI pulls on the way in that this file did not anticipate still
// has to construct and chain. The proxy is the backstop, not the API.
module.exports = new Proxy(known, {
  get(target, prop, receiver) {
    if (prop in target) return Reflect.get(target, prop, receiver);
    if (prop === "__esModule") return true;
    if (typeof prop === "symbol") return undefined;

    const dummy = new BaseAnimationMock();
    // Also callable/constructable for `new Keyframe(...)`-shaped imports.
    const fn = function Dummy() {
      return dummy;
    };
    fn.prototype = dummy;
    Object.setPrototypeOf(fn, dummy);
    return fn;
  },
  has() {
    return true;
  },
});
