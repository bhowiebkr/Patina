# Hello

The smallest Patina project. Two numbers go into an add node, and a show node writes the sum.
Start with this example to learn the parts of a project.

![The hello graph in the editor](images/editor.png)

## Run the example

Run these commands from the root of the repository.

- Run the graph one time:

  ```
  cargo run -p patina-cli -- run examples/hello
  ```

- Run the graph for 3 frames:

  ```
  cargo run -p patina-cli -- run examples/hello --frames 3
  ```

- Open the editor, then open the `examples/hello` folder. Click **Run**.

The show node writes `6.5`, which is 2.5 + 4.

## The parts

| Path | What it is |
| --- | --- |
| `patina.toml` | The project file. It sets the Rust version. |
| `graphs/main.toml` | The main graph: the nodes, their parameters and the wires. |
| `nodes/number/` | A node with a **value** parameter. It gives the value at its **out** output. |
| `nodes/add/` | A node that adds its inputs **a** and **b**. |
| `nodes/show/` | A node that writes its **value** input to the output. |

Each node is a Rust crate with one function or struct, marked with `#[node]`. Open a node in
the editor to see its code. Change the code, and the editor builds and runs the graph again.

## Try this

- Select a number node, and set its **value** in the **Inspector**.
- Make a package of the example:

  ```
  cargo run -p patina-cli -- package examples/hello hello-pkg
  ```
