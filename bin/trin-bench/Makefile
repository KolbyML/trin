generateFlameGraph:
	@echo "Generating flame graph for receiver..."
	@sudo chmod -R 777 flamegraph_receiver flamegraph_sender
	@cd flamegraph_receiver && perf script | inferno-collapse-perf | inferno-flamegraph > flamegraph.svg && cd ..
	@echo "Generating flame graph for sender..."
	@cd flamegraph_sender && perf script | inferno-collapse-perf | inferno-flamegraph > flamegraph.svg && cd ..
	@echo "Flame graph generated."

clean:
	sudo rm -r flamegraph_receiver flamegraph_sender flamegraph.svg stacks.folded