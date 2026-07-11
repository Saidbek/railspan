# frozen_string_literal: true

User.destroy_all

10.times do |i|
  user = User.create!(name: "User #{i}", email: "user#{i}@example.com")
  5.times do |j|
    user.posts.create!(title: "Post #{j} by #{user.name}", body: "Body #{j}")
  end
end

puts "Seeded #{User.count} users and #{Post.count} posts"
