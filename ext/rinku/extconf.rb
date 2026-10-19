require 'mkmf'
require 'rb_sys/mkmf'

create_rust_makefile('rinku') do |config|
  config.profile = ENV.fetch('RB_SYS_CARGO_PROFILE', 'release').to_sym
  config.extra_cargo_args = ['--locked']
end
