# Obsession (MVP)

Busca la mejor compra, no solo el precio más bajo.

```
cargo run --release -- "audífonos over-ear para música en casa, hasta 350 soles, Cusco"
```

Flujo: el **planificador** arma un dossier (necesidad, precio justo, candidatos, saltos de categoría, keywords). Después, un **experto por tienda** busca en paralelo con GET de solo lectura. Un **verificador** de otro modelo califica relevancia, confianza y costo real, y el **ranker** decide (o recomienda esperar). Todo queda en `obsession.db` y en `run-N.csv`.

- Tiendas: Amazon, AliExpress, Falabella y Plaza Vea. Mercado Libre y Temu bloquean con anti-bot; no se evade.
- Modelos vía el router magpie (`MAGPIE_URL`, por defecto `http://127.0.0.1:3425/v1`). Se cambian con `OBSESSION_MAIN` y `OBSESSION_VERIFIER`.
- Los prompts están en `prompts/` y se compilan dentro del binario, así que hay que recompilar después de editarlos.
