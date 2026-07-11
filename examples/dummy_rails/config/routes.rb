# frozen_string_literal: true

Rails.application.routes.draw do
  get "/up", to: "health#show"
  get "/health", to: "health#show"

  resources :users, only: %i[index show] do
    collection do
      get :with_posts
    end
  end
end
