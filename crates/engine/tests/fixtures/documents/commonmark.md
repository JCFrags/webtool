CommonMark field notes
======================

A *qualified* observation is **not a guarantee**. Keep `x_y`, \*literal stars\*, &amp; and [the guide][guide].
Two spaces make a hard break.  
This is the next line.

> A nested quotation keeps its scope.
>
> 3. Ordered item with **emphasis**.
>    - Nested item with [the guide][guide].
>    - A second item.
>
>    Continued paragraph for item three.
>
>    ```rust
>      let value = "`literal`";
>    ```
>
> 4. The next ordered item.

- [x] Supplied evidence
- [ ] Unavailable appendix

    Indented code keeps two trailing spaces.  
    Next code line.

A plot ![instrument plot](plot.svg "Plot title") has alt text but is not downloaded.
The supplied inline notation is $x^2 + y_1$. It is not evaluated.

$$
E = mc^2
$$

| Measurement | Value | Qualification |
| :--- | ---: | :---: |
| count | 0 | **literal zero** |
| absent | | no supplied value |

A statement with a footnote.[^scope]

[^scope]: Only this input was inspected.

---

<div data-note="raw">Raw HTML stays source syntax, not executed content.</div>

[guide]: https://example.test/guide "Source guide"
