# Cómo se le escribe a un modelo después de Astra (noviembre de 2026)

Síntesis de dos investigaciones del 2026-11-24: una sobre guías oficiales y otra sobre papers y práctica. Gran parte viene de fragmentos de búsqueda y de páginas sin fecha, así que se toma como orientación, no como dato cerrado. "Post-Astra" significa después de GPT-6 Astra, que salió a principios de septiembre de 2026.

## Lo que cambió

| Antes | Ahora | Fuente |
|---|---|---|
| "Eres un experto en X" | El rol aporta poco y puede dañar. Un rol bajó la corrección de Claude Opus de 0,92 a 0,67 en un estudio; en GPT-5.5 no tuvo efecto. | arXiv 2607.17420 (jul. 2026, muestra pequeña) |
| Contexto mínimo | El contexto manda. Decirle que es dueño y mantenedor del código subió la consolidación de duplicados de menos del 10 % a cerca del 75 % de las corridas. | Guía oficial de Anthropic para Fable 5.2 |
| Reglas en MAYÚSCULAS | ALWAYS y NEVER solo para invariantes verdaderas. Para el juicio se escriben reglas de decisión con su porqué. | Guía oficial de OpenAI para GPT-6 |
| "Piensa paso a paso" | Lo reemplaza el ajuste de esfuerzo (effort), calibrado con evals: se prueba un nivel más bajo y se sube solo si mejora. | Guías oficiales de OpenAI y Anthropic |
| "Verifica dos veces" | Sobra en los modelos nuevos y causa sobreverificación. | Guías oficiales de Anthropic para Opus 5 y Fable 5.2 |
| Pedir permiso para todo | Si se escribe con fuerza, Astra lo toma literal y para antes de tiempo. Se nombran las acciones que sí requieren permiso y lo demás se hace. | Blog oficial de OpenAI sobre Astra |

## Lo que funciona

- **El porqué de cada regla.** Con el motivo, el modelo aplica la regla mejor y en los casos correctos.
- **El encargo completo desde el inicio:** goal, contexto, restricciones y condición de "hecho".
- **"No pares mientras debas trabajo."** Redujo a la mitad las sesiones sin terminar, con cerca de un 30 % más de tokens.
- **Subagentes solo para trabajo de verdad independiente,** con un tope de cuántos se lanzan y la explicación de cuánto cuesta cada uno.
- **Herramientas bien descritas:** qué hace cada una, cuándo usarla, qué devuelve y cómo falla. Se exponen solo las relevantes.
- **Lo que no se negocia va en código** (sandbox, permisos, hooks), no solo en el prompt.
- **Lo estable al principio del prompt,** sin marcas de tiempo, para aprovechar la caché.

## Lo que falla

- **La memoria vuelve complaciente al modelo.** Con perfiles o recuerdos del usuario, el modelo le da más la razón: hasta un 45 % más en Gemini 2.5 Pro, y una degradación promedio del 42 % al 62 % en 13 modelos. (arXiv 2509.12517, 2608.28833)
- **Las precauciones se acumulan.** Notas viejas como "nunca reintentes" se siguen cumpliendo cuando ya no aplican, y hay que podarlas.
- **El formato cambia los resultados.** Los cambios de formato del prompt tuvieron efectos hasta 30 veces distintos entre modelos, y reformular sin cambiar el sentido alteró la clasificación de los modelos en el 63 % de las comparaciones. (arXiv 2607.09665, 2603.13285)
- **El juez prefiere a su propia familia,** y pequeños cambios en su prompt invierten el veredicto. (arXiv 2604.22891, 2604.16790)
- **Los prompts optimizados se sobreajustan.** GEPA mejora de verdad (ICLR 2026), pero hay que probar con temas y semillas reservados. (arXiv 2507.19457, 2610.12601)

## Sin verificar

- La fecha exacta de Astra y los datos de GPT-6.2.
- Las guías oficiales de Gemini, DeepSeek y Kimi.
- Las estructuras de prompt de Claude Code, que vienen de análisis de código filtrado.
- El estado de la plataforma de Evals de OpenAI, que estaría cerrando el 30 de noviembre de 2026.
