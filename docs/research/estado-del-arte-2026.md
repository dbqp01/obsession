# Estado del arte para Obsession (noviembre de 2026)

Barrido del 2026-11-24 con 8 investigadores Haiku, 8 verificadores y un crítico de completitud: 17 agentes, unas 190 fuentes. "Verificado" significa que un verificador abrió la fuente y confirmó la afirmación, o que yo la abrí. "Fragmento" significa que solo se vio en un resultado de búsqueda. Complementa a [prompting-2026.md](prompting-2026.md).

## 1. Por qué un agente sigue una regla que ya quitaste

Los cuatro estudios de esta sección explican el caso de un agente que insiste en una herramienta o un primer paso que el usuario ya descartó.

| Estudio | Qué encontró | Estado |
|---|---|---|
| [Stale Instruction Files Override the Code](https://arxiv.org/abs/2610.30571) (Xu y Wu, oct. 2026) | En 320 corridas con Claude Code y Codex CLI, el 90,1 % de las decisiones siguió un CLAUDE.md o AGENTS.md desactualizado frente al código real. Claude Code notó el conflicto en 79 de 80 corridas y aun así obedeció al archivo. La causa: el archivo llega como instrucción y el código como salida de herramienta, que el modelo aprendió a valorar menos. Una sola frase que da precedencia al código existente subió el seguimiento del código del 4,2 % al 91,4 %, y los tests pasados del 89,4 % al 100 %. | Verificado por mí. La frase exacta está en el PDF o en github.com/WenJing95/stale-docs-data. |
| [The Memory Trap](https://arxiv.org/abs/2611.07554) (Liang, Fisac, Griffiths y Lampinen, nov. 2026) | Una nota de memoria escrita en un contexto inofensivo se aplica después donde no corresponde. "No me confirmes las tareas menores" terminó en saltarse confirmaciones y verificación en tareas de alto riesgo. Notas de corregir tests viejos llevaron a debilitar tests correctos, y el efecto fue mayor en los modelos más capaces. Falla al escribir la nota, con una regla demasiado amplia, y al usarla, fuera de su alcance. | Verificado por mí. El abstract no da cifras. |
| [Why Does CLAUDE.md Keep Growing?](https://arxiv.org/abs/2608.11095) (Chakrabarti, ago. 2026) | En 1.867 repositorios, los archivos de instrucciones casi nunca encogen: crecen un 226 % en su vida útil. Agregar una regla es barato, y quitarla da miedo cuando nadie recuerda por qué existe. Comentar el porqué de cada regla eliminó el 99,3 % del crecimiento sobrante y mejoró el seguimiento de instrucciones en un 17,7 %. | Verificado por mí. |
| [Control Illusion](https://arxiv.org/abs/2502.15851) (AAAI-26) y [ManyIH-Bench](https://arxiv.org/abs/2604.09443) (abr. 2026) | Separar system y user no crea una jerarquía confiable, y un encuadre de autoridad o consenso puede ganarle al rol. Con hasta 12 niveles de instrucciones en conflicto, los modelos frontier aciertan cerca del 40 %. | Verificados |
| [Do Coding Agents Follow AGENTS.md?](https://arxiv.org/html/2611.04367) (nov. 2026) | Una regla prescriptiva se sigue más que la misma regla escrita como condicional ("si X, haz Y"). | Verificado. No da porcentajes. |

**Para Obsession:** pocas capas de instrucciones, cada regla con su porqué escrito, la precedencia declarada en una frase, memoria con alcance explícito (dónde y cuándo aplica) y poda periódica. El relato ya pide lo primero: las leyes del taller con su razón.

## 2. ¿El harness importa tanto como el modelo?

Esta es la evidencia que más pesa contra la tesis.

| Estudio | Qué encontró | Estado |
|---|---|---|
| [Harness or Model?](https://arxiv.org/abs/2609.11987) (Arjmandi, sep. 2026) | En 80 tareas privadas de código sin contaminación, cambiar de harness movió la tasa de resolución en cerca de 1 punto, y cambiar de modelo en 5 a 7 puntos. El promedio esconde efectos opuestos por tipo de tarea: en Opus, el harness nativo iba 9 puntos detrás en tareas de repositorio y 23,7 puntos delante en problemas de concurso. Ese corte se eligió después de ver los datos. | Verificado por mí |
| [The Scaffold Effect](https://arxiv.org/abs/2607.22585) (2026) | La diferencia en tasa de resolución entre harnesses fue de 0 a 8 puntos, pero el costo en tokens por tarea resuelta varió hasta 40 veces. | Verificado |
| [Multi-agent research system](https://www.anthropic.com/research/multi-agent-research-system) (Anthropic, 2025) | En investigación, el sistema multiagente superó a un solo Opus 4 en un 90,2 %, gastando cerca de 15 veces más tokens. El uso de tokens explicó el 80 % de la variación. | Verificado |
| [Terminal-Bench, mismo Claude con distinto harness](https://dev.to/gabrielanhaia/stop-comparing-models-same-claude-58-or-76-on-terminal-bench-depending-on-the-harness-16d1) | 58 % frente a 76 %, según el harness. | Fragmento, sin verificar |

**Para Obsession:** en código medido con tests, el harness cambia poco el resultado y mucho el costo. En investigación, donde el método y el volumen importan, la diferencia es grande, pero en buena parte se compra con tokens. La tesis "un modelo modesto con Obsession supera a uno brillante sin él" no está demostrada en general. Hay que probarla en los oficios de Obsession, con control, y reportar el costo junto al resultado.

## 3. Memoria

- [PersistBench](https://arxiv.org/html/2602.01146v2): con memorias del usuario, la complacencia tuvo una tasa mediana de fallo del 97 %, y la fuga de información irrelevante, del 53 %. Verificado.
- [Recalling Too Well](https://arxiv.org/html/2606.10949v2): la memoria persistente generó hasta un 40 % más de complacencia que el contexto en la misma conversación. Fragmento.
- [OP-Bench](https://arxiv.org/abs/2601.13722): sobrepersonalización, es decir, usar recuerdos que no hacían falta. Verificado.
- Envenenamiento de memoria: varios papers de 2026 reportan tasas de inyección altas. Los títulos están confirmados, pero las cifras no.

**Para Obsession:** la memoria decide el cómo (lenguaje, ritmo, intereses), pero no el juicio. El verificador trabaja sin el perfil del usuario, y cada nota guarda su alcance y su origen.

## 4. Contexto

- [Context Rot](https://www.trychroma.com/research/context-rot) (Chroma): en 18 modelos, el rendimiento cae al crecer la entrada, incluso en tareas simples.
- [IFScale](https://arxiv.org/abs/2507.11538): con 500 instrucciones a la vez, el mejor modelo cumplió el 68 %.
- [Agentic Context Engineering](https://arxiv.org/abs/2510.04618): un contexto que evoluciona como manual de estrategias, en lugar de un prompt fijo, mejoró un 10,6 % en agentes y un 8,6 % en finanzas. Verificado.
- [Effective context engineering](https://www.anthropic.com/research/effective-context-engineering-for-ai-agents) (Anthropic): los subagentes gastan decenas de miles de tokens y devuelven resúmenes de 1.000 a 2.000. Verificado.

## 5. Evaluar contra un control

- [Demystifying evals for AI agents](https://www.anthropic.com/research/demystifying-evals-for-ai-agents) (Anthropic, ene. 2026): bastan de 20 a 50 tareas iniciales. Mide pass^k, que todas las k corridas pasen, además de pass@k: con un 75 % de éxito por corrida, solo el 42 % de las veces pasan las 3. Verificado.
- [On Randomness in Agentic Evals](https://arxiv.org/pdf/2602.07150): una sola corrida puede variar de 2 a 6 puntos. Título verificado, cifras no.
- [Stochasticity in Agentic Evaluations](https://arxiv.org/abs/2512.06710): la consistencia entre corridas es baja o media (ICC de 0,30 a 0,77 en GAIA). Verificado.
- Los sesgos del juez (de posición, de familia y de autoridad) están bien documentados. Ver [prompting-2026.md](prompting-2026.md).

**Para Obsession:** la prueba de 5 agentes contra 1 de control mide ruido si cada uno corre una sola vez. Hacen falta varias corridas por lado, comparación en pares con el orden al azar y un juez de otra familia.

## 6. Automejora

- [GEPA](https://arxiv.org/abs/2507.19457) (ICLR 2026): la evolución de prompts por reflexión supera al aprendizaje por refuerzo (GRPO) con hasta 35 veces menos corridas. Verificado.
- [Darwin Gödel Machine](https://arxiv.org/abs/2505.22954): un agente que reescribe su propio código pasó del 20 % al 50 % en SWE-bench. Verificado.
- [Topic-disjoint protocol](https://arxiv.org/html/2610.12601v1): para medir sin sobreajuste se reservan temas y semillas que el optimizador nunca vio. Fragmento.
- [Hermes agent self-evolution](https://github.com/NousResearch/hermes-agent-self-evolution): usa DSPy y GEPA sobre skills y prompts. Repo, sin evaluación publicada.

## 7. Enseñar

- [Generative AI without guardrails can harm learning](https://pmc.ncbi.nlm.nih.gov/articles/PMC12232635/) (Bastani et al.): con GPT sin restricciones, los estudiantes rindieron peor en el examen (17 % menos, según el fragmento). Un tutor con guardas no causó ese daño. Existencia verificada, cifra no.
- [AI Assistance Reduces Persistence](https://arxiv.org/html/2604.04721) (abr. 2026): la ayuda de la IA reduce la persistencia y el desempeño independiente. Fragmento.
- [Khanmigo, experimento de dos años](https://www.nber.org/papers/w35620) (NBER, ago. 2026): un efecto pequeño, cerca de 0,06 a 0,08 desviaciones estándar por año. Verificado.
- [LearnLM](https://arxiv.org/abs/2412.16429v3): expertos en pedagogía prefirieron LearnLM sobre GPT-4o (+31 %). Verificado.
- Socrático contra no socrático: un estudio no encontró una ganancia significativa. Fragmento.
- Tutores complacientes: la precisión cae hasta 15 puntos cuando el estudiante sugiere una respuesta equivocada. Fragmento.

**Para Obsession:** dar la respuesta daña el aprendizaje, y eso apoya el redescubrimiento. Pero los tutores reales logran efectos pequeños, y el método socrático por sí solo no basta. La medida debe ser la transferencia a un caso nuevo, nunca la satisfacción.

## Repos útiles

[stanford-oval/storm](https://github.com/stanford-oval/storm), [langchain-ai/open_deep_research](https://github.com/langchain-ai/open_deep_research), [stanfordnlp/dspy](https://github.com/stanfordnlp/dspy), [NousResearch/hermes-agent](https://github.com/NousResearch/hermes-agent), [UKGovernmentBEIS/inspect_ai](https://github.com/UKGovernmentBEIS/inspect_ai), [promptfoo/promptfoo](https://github.com/promptfoo/promptfoo), [ytyz1307zzh/IHEval](https://github.com/ytyz1307zzh/IHEval).

## Pendiente de leer completo

- La frase exacta del paper de instrucciones desactualizadas.
- Las cifras del Memory Trap.
- [Who is In Charge? Role Conflicts](https://arxiv.org/html/2510.01228).
- [Where Instruction Hierarchy Breaks](https://arxiv.org/pdf/2606.07808).
- [Agents Don't Just Agree, They Remember](https://arxiv.org/pdf/2607.10526v4).
- [Evaluating AGENTS.md](https://arxiv.org/abs/2602.11988): los archivos de contexto suman más del 20 % de costo de inferencia.
