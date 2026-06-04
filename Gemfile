source "https://rubygems.org"

gem "fastlane", "~> 2.230", "< 2.235"

plugins_path = File.join(File.dirname(__FILE__), 'android', 'fastlane', 'Pluginfile')
eval_gemfile(plugins_path) if File.exist?(plugins_path)
