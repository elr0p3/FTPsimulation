.PHONI := c r

c: 
	@if [ ! -d build/ ]; then mkdir build/; fi
	javac src/r0p3/*.java -d ./build/


r: 
	java -classpath ./build/ r0p3.Main localhost 8080


clean:
	rm -rf ./build/*
