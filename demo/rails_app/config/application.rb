# Marks the folder as a Rails app, so the extension reads config/locales for Rails/MissingTranslation.
module Demo
  class Application < Rails::Application
    config.i18n.default_locale = :en
  end
end
