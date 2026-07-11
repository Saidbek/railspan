# frozen_string_literal: true

require_relative "test_helper"

class SqlNormalizerTest < Minitest::Test
  def test_replaces_string_and_number_literals
    sql = "SELECT * FROM users WHERE id = 42 AND email = 'a@b.com'"
    out = Railspan::SqlNormalizer.normalize(sql)
    assert_equal "SELECT * FROM users WHERE id = ? AND email = ?", out
  end

  def test_collapses_in_lists
    sql = "SELECT * FROM users WHERE id IN (1, 2, 3)"
    out = Railspan::SqlNormalizer.normalize(sql)
    assert_equal "SELECT * FROM users WHERE id IN (?)", out
  end

  def test_truncates_long_sql
    sql = "SELECT #{'x' * 5000}"
    out = Railspan::SqlNormalizer.normalize(sql)
    assert_operator out.length, :<=, Railspan::SqlNormalizer::MAX_LENGTH
  end

  def test_keeps_double_quoted_identifiers
    sql = 'SELECT "users".* FROM "users" WHERE "users"."id" = 42'
    out = Railspan::SqlNormalizer.normalize(sql)
    assert_equal 'SELECT "users".* FROM "users" WHERE "users"."id" = ?', out
  end
end
