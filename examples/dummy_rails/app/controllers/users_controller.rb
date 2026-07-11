# frozen_string_literal: true

class UsersController < ApplicationController
  def index
    users = User.limit(50)
    render json: users.as_json(only: %i[id name email])
  end

  def show
    user = User.find(params[:id])
    render json: user.as_json(only: %i[id name email])
  end

  # Intentional N+1 for later detector demos
  def with_posts
    users = User.limit(20)
    payload = users.map do |u|
      {
        id: u.id,
        name: u.name,
        posts: u.posts.map { |p| { id: p.id, title: p.title } }
      }
    end
    render json: payload
  end
end
