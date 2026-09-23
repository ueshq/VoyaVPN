# frozen_string_literal: true

# Wires `apps/mobile/ios/VoyaVPN.xcodeproj` up to everything this repo builds
# for it: the Swift/ObjC native module, the uniffi bindings and their static
# library, the shared PacketTunnel provider as an app extension, and a UI test
# bundle for the simulator checks.
#
# Idempotent by construction — every step finds the object it wants before it
# makes one, and settings are assigned rather than appended. That matters
# because `pod install` and a React Native upgrade both rewrite parts of this
# project, and running this again is how our half comes back.
#
# Run it through `pnpm run native:mobile:ios:project`, which finds a Ruby that
# can load the `xcodeproj` gem.

require 'shellwords'
require 'xcodeproj'

APP_TARGET = 'VoyaVPN'
APPEX_TARGET = 'PacketTunnel'
UI_TEST_TARGET = 'VoyaVPNUITests'

APP_BUNDLE_ID = 'app.voyavpn.mobile'
# Never the desktop's `app.voyavpn.desktop.PacketTunnel`: macOS elects
# app-extension providers globally by bundle id through PlugInKit, so a second
# bundle claiming that id can become the elected provider for the desktop app.
APPEX_BUNDLE_ID = "#{APP_BUNDLE_ID}.PacketTunnel"
UI_TEST_BUNDLE_ID = "#{APP_BUNDLE_ID}.uitests"

DEPLOYMENT_TARGET = '15.1'
SWIFT_VERSION = '5.0'
# `check:architecture` requires one MARKETING_VERSION across the whole project,
# so the extension carries the app's. The test bundle deliberately carries none.
MARKETING_VERSION = '0.1.0'
CURRENT_PROJECT_VERSION = '1'

repo_root = Dir.pwd
ios_root = File.join(repo_root, 'apps/mobile/ios')
project_path = File.join(ios_root, 'VoyaVPN.xcodeproj')
project = Xcodeproj::Project.open(project_path)

# --- helpers ----------------------------------------------------------------

# A group whose files live outside the project directory, addressed relative to
# it. Used for the provider sources, which are shared with the macOS app and
# must be referenced in place so the two cannot drift.
def group_at(project, name, path, source_tree: 'SOURCE_ROOT')
  group = project.main_group.children.find { |child| child.display_name == name }
  group ||= project.main_group.new_group(name)
  group.set_source_tree(source_tree)
  group.set_path(path)
  group
end

def file_ref(group, path)
  group.files.find { |file| file.path == path } || group.new_reference(path)
end

def ensure_source(target, ref)
  phase = target.source_build_phase
  return if phase.files_references.include?(ref)

  phase.add_file_reference(ref)
end

def ensure_framework(target, ref)
  phase = target.frameworks_build_phase
  return if phase.files_references.include?(ref)

  phase.add_file_reference(ref)
end

def sdk_framework(project, name)
  path = "System/Library/Frameworks/#{name}.framework"
  ref = project.frameworks_group.files.find { |file| file.path == path }
  return ref if ref

  ref = project.frameworks_group.new_reference(path)
  ref.source_tree = 'SDKROOT'
  ref
end

# Appends a linker flag without losing what is already there — the app's own
# `-ObjC`/`-lc++`, and whatever the Pods xcconfig contributes through
# `$(inherited)`.
def append_ldflag(target, flag)
  target.build_configurations.each do |config|
    existing = config.build_settings['OTHER_LDFLAGS'] || ['$(inherited)']
    existing = [existing] unless existing.is_a?(Array)
    existing = ['$(inherited)'] + existing unless existing.include?('$(inherited)')
    config.build_settings['OTHER_LDFLAGS'] = existing + [flag] unless existing.include?(flag)
  end
end

def apply_settings(target, settings)
  target.build_configurations.each do |config|
    settings.each do |key, value|
      if value.nil?
        config.build_settings.delete(key)
      else
        config.build_settings[key] = value
      end
    end
  end
end

# A gomobile xcframework can carry either a static archive or a dynamic
# framework depending on how it was built, and only a dynamic one has to be
# embedded in the app bundle. Asking the binary is more reliable than assuming.
def dynamic_xcframework?(path)
  binary = Dir.glob(File.join(path, '*', '*.framework', '*')).find do |candidate|
    File.file?(candidate) && File.basename(candidate) == File.basename(candidate, '.*')
  end
  return false unless binary

  `file #{binary.shellescape} 2>/dev/null`.include?('dynamically linked shared library')
end

# --- the app target ---------------------------------------------------------

app = project.targets.find { |target| target.name == APP_TARGET }
raise "No #{APP_TARGET} target in #{project_path}" unless app

# The template's `VoyaVPN` group carries a `name`, not a `path`, so its
# children spell the directory out themselves. New subgroups have to do the
# same or their files resolve a level too high.
app_group = project.main_group.children.find { |child| child.display_name == APP_TARGET }
native_group = app_group.children.find { |child| child.display_name == 'Native' } ||
               app_group.new_group('Native', "#{APP_TARGET}/Native")
generated_group = app_group.children.find { |child| child.display_name == 'Generated' } ||
                  app_group.new_group('Generated', "#{APP_TARGET}/Generated")

%w[VoyaNative.swift VoyaNative.m SystemTunnelHost.swift LibboxProbeCoreHost.swift].each do |name|
  ensure_source(app, file_ref(native_group, name))
end
# Hand-written, not generated: it is what lets the generated Swift see the C
# symbols (see SWIFT_OBJC_BRIDGING_HEADER below).
file_ref(native_group, 'VoyaVPN-Bridging-Header.h')
ensure_source(app, file_ref(generated_group, 'voya_mobile_ffi.swift'))

frameworks_group = group_at(project, 'VoyaFrameworks', 'Frameworks')
voya_mobile = file_ref(frameworks_group, 'VoyaMobile.xcframework')
ensure_framework(app, voya_mobile)
ensure_framework(app, sdk_framework(project, 'NetworkExtension'))

libbox_path = File.join(ios_root, 'Frameworks/Libbox.xcframework')
libbox = nil
if File.directory?(libbox_path)
  libbox = file_ref(frameworks_group, 'Libbox.xcframework')
  ensure_framework(app, libbox)
  # Go's resolver calls into libresolv (`res_9_ninit` and friends), which iOS
  # does not link by default.
  append_ldflag(app, '-lresolv')
  # The Go runtime brings more personality routines than the compact unwind
  # format can encode, and the linker refuses rather than falling back. The
  # DWARF tables it then emits are what every gomobile consumer ships.
  append_ldflag(app, '-Wl,-no_compact_unwind')
else
  warn "Libbox.xcframework is not built; skipping it. Run `pnpm native:mobile:libbox:ios`, then this again."
end

apply_settings(app, {
  'FRAMEWORK_SEARCH_PATHS' => ['$(inherited)', '$(SRCROOT)/Frameworks'],
  # The generated Swift does `#import`-free `canImport(voya_mobile_ffiFFI)`,
  # which is false here because the modulemap is not named `module.modulemap`.
  # The bridging header is what supplies `RustBuffer` and the `uniffi_*`
  # symbols instead. Exactly one of the two may be active: both would define
  # the same C declarations twice.
  'HEADER_SEARCH_PATHS' => ['$(inherited)', '$(SRCROOT)/VoyaVPN/Generated'],
  'SWIFT_OBJC_BRIDGING_HEADER' => 'VoyaVPN/Native/VoyaVPN-Bridging-Header.h',
  'CODE_SIGN_ENTITLEMENTS' => 'VoyaVPN/VoyaVPN.entitlements'
})

# --- the PacketTunnel extension ---------------------------------------------

appex = project.targets.find { |target| target.name == APPEX_TARGET }
appex ||= project.new_target(:app_extension, APPEX_TARGET, :ios, DEPLOYMENT_TARGET, nil, :swift)

provider_group = group_at(project, 'PacketTunnelProvider', '../../../native/apple/PacketTunnel')
%w[
  PacketTunnelProvider.swift
  PacketTunnelRuntime.swift
  PacketTunnelDiagnostics.swift
  PacketTunnelPlatform.swift
].each { |name| ensure_source(appex, file_ref(provider_group, name)) }

appex_group = project.main_group.children.find { |child| child.display_name == APPEX_TARGET } ||
              group_at(project, APPEX_TARGET, APPEX_TARGET)
file_ref(appex_group, 'Info.plist')
file_ref(appex_group, 'PacketTunnel.entitlements')

ensure_framework(appex, sdk_framework(project, 'NetworkExtension'))
if libbox
  # Libbox's Chromium base pulls in UIApplication / UIBackgroundTaskInvalid /
  # UIDevice (scoped_critical_action, MessagePumpUIApplication). The extension
  # sources never `import UIKit`, so Swift autolinking does not add it and the
  # PacketTunnel link fails with those symbols missing.
  ensure_framework(appex, sdk_framework(project, 'UIKit'))
  ensure_framework(appex, libbox)
  append_ldflag(appex, '-lresolv')
  append_ldflag(appex, '-Wl,-no_compact_unwind')
end

apply_settings(appex, {
  # `Info.plist` resolves the provider as `$(PRODUCT_MODULE_NAME).PacketTunnelProvider`,
  # so the module name is load-bearing.
  'PRODUCT_NAME' => APPEX_TARGET,
  'PRODUCT_MODULE_NAME' => APPEX_TARGET,
  'PRODUCT_BUNDLE_IDENTIFIER' => APPEX_BUNDLE_ID,
  'INFOPLIST_FILE' => 'PacketTunnel/Info.plist',
  'CODE_SIGN_ENTITLEMENTS' => 'PacketTunnel/PacketTunnel.entitlements',
  'SWIFT_VERSION' => SWIFT_VERSION,
  'IPHONEOS_DEPLOYMENT_TARGET' => DEPLOYMENT_TARGET,
  'TARGETED_DEVICE_FAMILY' => '1,2',
  'MARKETING_VERSION' => MARKETING_VERSION,
  'CURRENT_PROJECT_VERSION' => CURRENT_PROJECT_VERSION,
  'VERSIONING_SYSTEM' => 'apple-generic',
  'FRAMEWORK_SEARCH_PATHS' => ['$(inherited)', '$(SRCROOT)/Frameworks'],
  # The extension lives two levels down in PlugIns, so a framework the app
  # embedded is up there rather than beside it.
  'LD_RUNPATH_SEARCH_PATHS' => [
    '$(inherited)',
    '@executable_path/Frameworks',
    '@executable_path/../../Frameworks'
  ],
  'SKIP_INSTALL' => 'YES'
})

app.add_dependency(appex)
embed = app.copy_files_build_phases.find { |phase| phase.name == 'Embed App Extensions' } ||
        app.new_copy_files_build_phase('Embed App Extensions')
embed.symbol_dst_subfolder_spec = :plug_ins
unless embed.files_references.include?(appex.product_reference)
  embed.add_file_reference(appex.product_reference).settings = {
    'ATTRIBUTES' => ['RemoveHeadersOnCopy']
  }
end
# Where Xcode itself puts it: after the app's own resources, before the pods
# phases that follow.
resources_index = app.build_phases.index(app.resources_build_phase)
if resources_index && app.build_phases.index(embed) != resources_index + 1
  app.build_phases.delete(embed)
  app.build_phases.insert(resources_index + 1, embed)
end

if libbox && dynamic_xcframework?(libbox_path)
  frameworks = app.copy_files_build_phases.find { |phase| phase.name == 'Embed Frameworks' } ||
               app.new_copy_files_build_phase('Embed Frameworks')
  frameworks.symbol_dst_subfolder_spec = :frameworks
  unless frameworks.files_references.include?(libbox)
    frameworks.add_file_reference(libbox).settings = {
      'ATTRIBUTES' => %w[CodeSignOnCopy RemoveHeadersOnCopy]
    }
  end
end

# --- the UI test bundle -----------------------------------------------------

ui_tests = project.targets.find { |target| target.name == UI_TEST_TARGET }
ui_tests ||= project.new_target(:ui_test_bundle, UI_TEST_TARGET, :ios, DEPLOYMENT_TARGET, nil, :swift)

ui_group = group_at(project, UI_TEST_TARGET, UI_TEST_TARGET)
Dir.glob(File.join(ios_root, UI_TEST_TARGET, '*.swift')).sort.each do |path|
  ensure_source(ui_tests, file_ref(ui_group, File.basename(path)))
end

apply_settings(ui_tests, {
  'PRODUCT_NAME' => UI_TEST_TARGET,
  'PRODUCT_BUNDLE_IDENTIFIER' => UI_TEST_BUNDLE_ID,
  'TEST_TARGET_NAME' => APP_TARGET,
  'GENERATE_INFOPLIST_FILE' => 'YES',
  'SWIFT_VERSION' => SWIFT_VERSION,
  'IPHONEOS_DEPLOYMENT_TARGET' => DEPLOYMENT_TARGET,
  'TARGETED_DEVICE_FAMILY' => '1,2',
  'CODE_SIGN_STYLE' => 'Automatic',
  # A test bundle's release version means nothing, and setting one would break
  # the single-MARKETING_VERSION rule `check:architecture` enforces.
  'MARKETING_VERSION' => nil,
  'CURRENT_PROJECT_VERSION' => nil
})
ui_tests.add_dependency(app)

# `build-rust-ios.mjs` builds `aarch64-apple-ios` and `aarch64-apple-ios-sim`
# and nothing else, so an Intel simulator slice has no host to link against.
# Debug never noticed because it builds the active architecture only; Release
# builds both and fails at the link. Saying so at the project level keeps every
# target in step with the artifacts the repo actually produces.
project.build_configurations.each do |config|
  config.build_settings['EXCLUDED_ARCHS[sdk=iphonesimulator*]'] = 'x86_64'
end

project.save

# --- the shared scheme ------------------------------------------------------

# The template left a TestAction pointing at a `VoyaVPNTests.xctest` that never
# existed, which is why `xcodebuild test -scheme VoyaVPN` has never worked.
scheme_path = File.join(project_path, 'xcshareddata/xcschemes/VoyaVPN.xcscheme')
if File.exist?(scheme_path)
  scheme = Xcodeproj::XCScheme.new(scheme_path)
  scheme.test_action.testables = []
  scheme.add_test_target(ui_tests)
  scheme.save!
end

puts "Wired #{project_path}:"
puts "  #{APP_TARGET}: native module + uniffi bindings + VoyaMobile#{libbox ? ' + Libbox' : ''}"
puts "  #{APPEX_TARGET}: #{APPEX_BUNDLE_ID}, provider shared from native/apple/"
puts "  #{UI_TEST_TARGET}: #{ui_tests.source_build_phase.files_references.count} test source(s)"
