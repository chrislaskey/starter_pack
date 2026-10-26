defmodule App.Data.UniversalChangesetTest do
  # A changeset function marked @universal_changeset true runs Rust, and its
  # Ecto pipeline stays in the source as the template the Rust was written
  # from. On every case here the two must give the same errors. Add a case
  # for every rule a marked function gains.
  use App.DataCase, async: true

  alias App.Data.{Post, UniversalChangeset}

  @author Ecto.UUID.generate()
  @long String.duplicate("x", 10_001)

  @cases [
    %{function: :changeset, name: "a post", params: %{"body" => "Hello"}},
    %{function: :changeset, name: "no params", params: %{}},
    %{function: :changeset, name: "no author", params: %{"body" => "Hello"}, options: []},
    %{function: :changeset, name: "blank body", params: %{"body" => "  "}},
    %{function: :changeset, name: "body too long", params: %{"body" => @long}},
    %{function: :changeset, name: "body at the limit", params: %{"body" => String.duplicate("x", 10_000)}},
    %{function: :changeset, name: "body not a string", params: %{"body" => 1}},
    %{function: :changeset, name: "atom keys", params: %{body: "Hello"}},
    %{
      function: :changeset,
      name: "editing keeps the author",
      data: %{author_id: @author, body: "Old"},
      params: %{"body" => "New"},
      options: []
    },
    %{
      function: :changeset,
      name: "an unchanged body is not validated",
      data: %{author_id: @author, body: @long},
      params: %{"body" => @long},
      options: []
    }
  ]

  defp cases do
    for case <- @cases do
      %{module: Post, data: %{}, options: [author_id: @author]} |> Map.merge(case)
    end
  end

  defp args(%{module: module, data: data, params: params, options: options}),
    do: [struct(module, data), params, options]

  test "the marked functions are the ones with cases here" do
    marked = for {module, sources} <- EctoRust.marked(:app), source <- sources, do: {module, source.function}
    with_cases = cases() |> Enum.map(&{&1.module, &1.function}) |> Enum.uniq()

    assert Enum.sort(marked) == Enum.sort(with_cases),
           "a marked function needs cases here, and a function no longer marked loses its cases"
  end

  test "the Rust gives the errors the Ecto template gives" do
    differences =
      for case <- cases(),
          template = UniversalChangeset.template(case.module, case.function, args(case)),
          rust = apply(case.module, case.function, args(case)),
          {errors(template), template.valid?} != {errors(rust), rust.valid?} do
        "#{inspect(case.module)}.#{case.function} #{case.name}:\n  template: #{inspect(errors(template))}\n  rust:     #{inspect(errors(rust))}"
      end

    assert differences == [], Enum.join(differences, "\n")
  end

  test "the changeset keeps what the template puts and what the repo reads" do
    changeset = Post.changeset(%Post{}, %{"body" => "Hello"}, author_id: @author)

    assert changeset.valid?
    assert changeset.changes.body == "Hello"
    assert changeset.changes.author_id == @author
    assert %DateTime{} = changeset.changes.last_synced_at
    assert changeset.required == [:author_id, :body]
    assert [{:body, {:length, [max: 10_000]}}] = changeset.validations
  end

  test "an error is what Ecto reports, template and options included" do
    changeset = Post.changeset(%Post{}, %{"body" => @long}, author_id: @author)

    assert changeset.errors == [
             body:
               {"should be at most %{count} character(s)",
                [count: 10_000, validation: :length, kind: :max, type: :string]}
           ]

    assert errors_on(changeset) == %{body: ["should be at most 10000 character(s)"]}
    refute changeset.valid?
  end

  # Errors of one validate_required call keep their field order in Ecto
  # and come reversed from the Rust, so they compare sorted
  defp errors(changeset), do: Enum.sort(changeset.errors)
end
