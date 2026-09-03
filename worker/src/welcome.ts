export function welcomePage(): string {
  const navbar = `
    <header class="navbar">
      <div class="brand">TexelBox</div>
      <nav class="nav-links">
        <a href="/pricing" class="nav-link">Products</a>
        <a href="/login" class="nav-link">Log in</a>
        <a href="/signup" class="nav-link nav-link--primary">Sign up</a>
      </nav>
    </header>
  `;

  const styles = `
    <style>
      .navbar {
        position: fixed;
        top: 0; left: 0; right: 0;
        height: 56px;
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0 24px;
        background: rgba(0, 0, 0, 0.55);
        backdrop-filter: blur(10px);
        -webkit-backdrop-filter: blur(10px);
        border-bottom: 1px solid rgba(255,255,255,0.08);
        z-index: 100;
      }
      .brand {
        font-family: system-ui, Segoe UI, Roboto, sans-serif;
        font-size: 18px;
        font-weight: 700;
        color: #fff;
        letter-spacing: -0.3px;
      }
      .nav-links {
        display: flex;
        gap: 10px;
        align-items: center;
      }
      .nav-link {
        color: rgba(255,255,255,0.9);
        text-decoration: none;
        font-size: 14px;
        font-weight: 500;
        padding: 7px 14px;
        border-radius: 6px;
        transition: background 0.15s, color 0.15s;
      }
      .nav-link:hover {
        background: rgba(255,255,255,0.12);
        color: #fff;
      }
      .nav-link--primary {
        background: #2563eb;
        color: #fff;
        padding: 7px 16px;
      }
      .nav-link--primary:hover {
        background: #1d4ed8;
      }
      body { margin: 0; overflow: hidden; }
      canvas#bg {
        position: fixed;
        top: 0; left: 0;
        width: 100vw; height: 100vh;
        z-index: 0;
      }
      .navbar { position: relative; z-index: 10; }
    </style>
  `;

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>TexelBox — Free Texture Tooling for Game Developers</title>
  <meta name="description" content="TexelBox — texture tooling for game developers. Generate normal, height, roughness, and AO maps. Build tileable textures and texture atlases. Native Windows app with free tier and 1-day Pro trial." />
  <meta name="keywords" content="texture atlas, tileable textures, seamless textures, normal map generator, roughness map, ambient occlusion AO map, height map, texture tool, game textures, atlas packing, channel packing, DDS compression, TexelBox, build atlas, tileable images, texture generation, PBR textures, texture baking, gamedev tools, Windows" />
  <meta name="google-site-verification" content="evJVZtiWOTEZZ4QAhMWDbK1H5UzQvB6VqmRgHbXI2U0" />
  <meta name="author" content="TexelBox" />
  <meta name="robots" content="index, follow, max-snippet:-1, max-image-preview:large, max-video-preview:-1" />
  <meta name="googlebot" content="index, follow, max-snippet:-1, max-image-preview:large, max-video-preview:-1" />
  <meta name="bingbot" content="index, follow, max-snippet:-1, max-image-preview:large" />
  <meta name="theme-color" content="#2563eb" />
  <meta name="format-detection" content="telephone=no" />
  <link rel="canonical" href="https://texelbox-license.imadedar98.workers.dev/" />
  <meta property="og:title" content="TexelBox — Free Texture Tooling for Game Developers" />
  <meta property="og:description" content="TexelBox — texture tooling for game developers. Generate normal, height, roughness, and AO maps. Build tileable textures and texture atlases. Native Windows app with free tier and 1-day Pro trial." />
  <meta property="og:image" content="https://raw.githubusercontent.com/iimadouu/TexelBox/master/texelbox.png" />
  <meta property="og:url" content="https://texelbox-license.imadedar98.workers.dev/" />
  <meta property="og:type" content="website" />
  <meta property="og:site_name" content="TexelBox" />
  <meta name="twitter:card" content="summary_large_image" />
  <meta name="twitter:title" content="TexelBox — Free Texture Tooling for Game Developers" />
  <meta name="twitter:description" content="TexelBox — texture tooling for game developers. Generate normal, height, roughness, and AO maps. Build tileable textures and texture atlases." />
  <meta name="twitter:image" content="https://raw.githubusercontent.com/iimadouu/TexelBox/master/texelbox.png" />
  <meta name="twitter:label1" content="Platform" />
  <meta name="twitter:data1" content="Windows" />
  <meta name="twitter:label2" content="Price" />
  <meta name="twitter:data2" content="Free" />
  <link rel="icon" href="https://raw.githubusercontent.com/iimadouu/TexelBox/master/texelbox.png" sizes="any" />
  <script type="application/ld+json">{"@context":"https://schema.org","@type":"WebSite","name":"TexelBox","url":"https://texelbox-license.imadedar98.workers.dev/","description":"TexelBox \u2014 texture tooling for game developers. Generate normal, height, roughness, and AO maps. Build tileable textures and texture atlases. Native Windows app.","sameAs":["https://github.com/iimadouu/TexelBox"]}</script>
  <script type="application/ld+json">{"@context":"https://schema.org","@type":"SoftwareApplication","name":"TexelBox","operatingSystem":"Windows","applicationCategory":"GraphicsSoftware","offers":{"@type":"Offer","price":"0","priceCurrency":"USD"},"description":"TexelBox \u2014 texture tooling for game developers. Generate normal, height, roughness, and AO maps. Build tileable textures and texture atlases. Native Windows app with free tier and 1-day Pro trial.","url":"https://texelbox-license.imadedar98.workers.dev/","publisher":{"@type":"Organization","name":"TexelBox","url":"https://github.com/iimadouu/TexelBox"}}</script>
  ${styles}
  <script src="https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js"></script>
</head>
<body>
  ${navbar}
  <script>
    (function() {
      const scene = new THREE.Scene();
      scene.background = new THREE.Color(0x000000);
      scene.fog = new THREE.FogExp2(0x000000, 0.03);

      const camera = new THREE.PerspectiveCamera(45, window.innerWidth / window.innerHeight, 0.1, 100);
      camera.position.set(0, 0.3, 4);

      const renderer = new THREE.WebGLRenderer({ antialias: true });
      renderer.setSize(window.innerWidth, window.innerHeight);
      renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
      const canvas = renderer.domElement;
      canvas.id = "bg";
      document.body.appendChild(canvas);

      const keyLight = new THREE.DirectionalLight(0xffffff, 2.0);
      keyLight.position.set(2, 3, 4);
      scene.add(keyLight);

      const rimLight = new THREE.DirectionalLight(0x00ffcc, 1.5);
      rimLight.position.set(-3, 1, -3);
      scene.add(rimLight);

      const ambient = new THREE.AmbientLight(0x111122, 0.5);
      scene.add(ambient);

      const particleCount = 900;
      const particleGeometry = new THREE.BufferGeometry();
      const particlePositions = new Float32Array(particleCount * 3);
      for (let i = 0; i < particleCount; i++) {
        const r = 6 + Math.random() * 16;
        const theta = Math.random() * Math.PI * 2;
        const phi = Math.acos(Math.random() * 2 - 1);
        particlePositions[i * 3]     = r * Math.sin(phi) * Math.cos(theta);
        particlePositions[i * 3 + 1] = r * Math.sin(phi) * Math.sin(theta);
        particlePositions[i * 3 + 2] = r * Math.cos(phi);
      }
      particleGeometry.setAttribute('position', new THREE.BufferAttribute(particlePositions, 3));

      const particleMaterial = new THREE.PointsMaterial({
        color: 0x66ffe0,
        size: 0.045,
        transparent: true,
        opacity: 0.5,
        sizeAttenuation: true,
        depthWrite: false,
        blending: THREE.AdditiveBlending,
        fog: true
      });

      const particles = new THREE.Points(particleGeometry, particleMaterial);
      scene.add(particles);

      const geometry = new THREE.SphereGeometry(1.0, 96, 96);

      const shaderMaterial = new THREE.ShaderMaterial({
        uniforms: {
          uTime: { value: 0.0 },
          uKeyDir: { value: new THREE.Vector3(0.5, 0.6, 0.8).normalize() },
          uRimDir: { value: new THREE.Vector3(-0.7, 0.2, -0.7).normalize() }
        },
        vertexShader: \`
          varying vec3 vNormal;
          varying vec3 vPosition;
          void main() {
            vNormal = normalize(normalMatrix * normal);
            vPosition = position;
            gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
          }
        \`,
        fragmentShader: \`
          precision highp float;

          uniform float uTime;
          uniform vec3 uKeyDir;
          uniform vec3 uRimDir;

          varying vec3 vNormal;
          varying vec3 vPosition;

          #define MATERIAL_COUNT 10
          #define NUM_STAGES 30

          float rand(vec3 p) {
            return fract(sin(dot(p, vec3(12.9898, 78.233, 37.719))) * 43758.5453);
          }

          vec3 fakeNormalMapColor(vec3 pos, float scale, float strength, float seedOffset) {
            float nx = rand(floor(pos * scale)) - 0.5;
            float ny = rand(floor(pos * scale) + seedOffset) - 0.5;
            vec3 n = normalize(vec3(nx * strength, ny * strength, 1.0));
            return n * 0.5 + 0.5;
          }

          vec3 materialAlbedo(int m, vec3 pos) {
            if (m == 0) {
              float noiseVal = rand(floor(pos * 6.0));
              vec3 base = mix(vec3(0.16, 0.42, 0.12), vec3(0.35, 0.60, 0.20), noiseVal);
              float streak = rand(floor(pos * 20.0));
              base += vec3(0.0, 0.05, 0.0) * streak;
              return base;
            }
            else if (m == 1) {
              float noiseVal = rand(floor(pos * 5.0));
              return mix(vec3(0.28, 0.18, 0.10), vec3(0.45, 0.32, 0.18), noiseVal);
            }
            else if (m == 2) {
              float noiseVal = rand(floor(pos * 8.0));
              return mix(vec3(0.88, 0.92, 0.98), vec3(0.98, 0.99, 1.0), noiseVal);
            }
            else if (m == 3) {
              float noiseVal = rand(floor(pos * 10.0));
              vec3 base = mix(vec3(0.82, 0.68, 0.45), vec3(0.93, 0.82, 0.60), noiseVal);
              float ripple = sin(pos.x * 15.0 + pos.z * 10.0) * 0.5 + 0.5;
              return mix(base, base * 1.05, ripple * 0.3);
            }
            else if (m == 4) {
              float noiseVal = rand(floor(pos * 4.0));
              vec3 base = mix(vec3(0.35, 0.35, 0.37), vec3(0.55, 0.54, 0.50), noiseVal);
              vec3 p = pos * 6.0;
              vec3 g = abs(fract(p) - 0.5);
              float crack = 1.0 - smoothstep(0.0, 0.03, min(g.x, min(g.y, g.z)));
              return base * (1.0 - crack * 0.5);
            }
            else if (m == 5) {
              float noiseVal = rand(floor(pos * 7.0));
              return mix(vec3(0.18, 0.12, 0.08), vec3(0.32, 0.22, 0.14), noiseVal);
            }
            else if (m == 6) {
              float rings = sin(pos.x * 25.0 + rand(floor(pos * 2.0)) * 3.0);
              rings = smoothstep(-0.2, 0.2, rings);
              return mix(vec3(0.35, 0.20, 0.10), vec3(0.55, 0.35, 0.18), rings);
            }
            else if (m == 7) {
              float noiseVal = rand(floor(pos * 20.0));
              vec3 base = mix(vec3(0.30, 0.31, 0.33), vec3(0.50, 0.51, 0.53), noiseVal);
              float scratch = smoothstep(0.95, 1.0, rand(floor(pos * 40.0) + 11.0));
              return base + vec3(0.15) * scratch;
            }
            else if (m == 8) {
              float noiseVal = rand(floor(pos * 9.0));
              vec3 base = mix(vec3(0.08, 0.28, 0.08), vec3(0.22, 0.45, 0.15), noiseVal);
              float clump = rand(floor(pos * 3.0));
              return base * (0.8 + 0.4 * clump);
            }
            else {
              vec3 base = vec3(0.75, 0.9, 1.0);
              float facet = rand(floor(pos * 8.0));
              vec3 tint = mix(base, vec3(0.35, 0.65, 0.9), facet);
              vec3 p = pos * 7.0;
              vec3 g = abs(fract(p) - 0.5);
              float crack = 1.0 - smoothstep(0.0, 0.02, min(g.x, min(g.y, g.z)));
              return mix(tint, vec3(1.0), crack * 0.3);
            }
          }

          vec2 materialNormalParams(int m) {
            if (m == 0) return vec2(14.0, 0.6);
            else if (m == 1) return vec2(10.0, 0.8);
            else if (m == 2) return vec2(16.0, 0.25);
            else if (m == 3) return vec2(20.0, 0.35);
            else if (m == 4) return vec2(8.0, 0.9);
            else if (m == 5) return vec2(12.0, 0.5);
            else if (m == 6) return vec2(6.0, 0.4);
            else if (m == 7) return vec2(30.0, 0.15);
            else if (m == 8) return vec2(18.0, 0.7);
            else return vec2(10.0, 0.2);
          }

          vec2 materialRoughnessRange(int m) {
            if (m == 0) return vec2(0.75, 0.95);
            else if (m == 1) return vec2(0.60, 0.95);
            else if (m == 2) return vec2(0.25, 0.60);
            else if (m == 3) return vec2(0.70, 0.90);
            else if (m == 4) return vec2(0.50, 0.80);
            else if (m == 5) return vec2(0.20, 0.60);
            else if (m == 6) return vec2(0.40, 0.70);
            else if (m == 7) return vec2(0.10, 0.40);
            else if (m == 8) return vec2(0.80, 1.00);
            else return vec2(0.05, 0.25);
          }

          vec3 getStageColor(int stage, vec3 pos, vec3 n) {
            int materialID = stage / 3;
            int mapType = stage - materialID * 3;

            if (mapType == 0) {
              return materialAlbedo(materialID, pos);
            } else if (mapType == 1) {
              vec2 p = materialNormalParams(materialID);
              return fakeNormalMapColor(pos, p.x, p.y, float(materialID) * 4.7 + 3.3);
            } else {
              vec2 r = materialRoughnessRange(materialID);
              float t = rand(floor(pos * (8.0 + float(materialID))));
              return vec3(mix(r.x, r.y, t));
            }
          }

          void main() {
            float cycleSeconds = 8.0 * float(NUM_STAGES) / 4.0;
            float phase = fract(uTime / cycleSeconds);

            float seg = phase * float(NUM_STAGES);
            float idx = floor(seg);
            float frac = seg - idx;

            int i0 = int(mod(idx, float(NUM_STAGES)));
            int i1 = int(mod(idx + 1.0, float(NUM_STAGES)));

            vec3 col0 = getStageColor(i0, vPosition, vNormal);
            vec3 col1 = getStageColor(i1, vPosition, vNormal);
            vec3 baseColor = mix(col0, col1, smoothstep(0.0, 1.0, frac));

            vec3 N = normalize(vNormal);
            float keyDiff = max(dot(N, uKeyDir), 0.0);
            float rimDiff = max(dot(N, uRimDir), 0.0);

            vec3 rimColor = vec3(0.0, 1.0, 0.8) * pow(rimDiff, 2.0) * 0.8;
            vec3 finalColor = baseColor * (0.25 + keyDiff * 0.9) + rimColor;

            gl_FragColor = vec4(finalColor, 1.0);
          }
        \`
      });

      const sphere = new THREE.Mesh(geometry, shaderMaterial);
      scene.add(sphere);

      let dragging = false;
      let lastPointerX = 0, lastPointerY = 0;

      const defaultVelY = 0.001;
      const defaultVelX = 0.0005;
      const dragSensitivity = 0.006;
      const easeBack = 0.06;

      let velY = defaultVelY;
      let velX = defaultVelX;

      const dom = renderer.domElement;

      function onPointerDown(e) {
        dragging = true;
        lastPointerX = e.clientX;
        lastPointerY = e.clientY;
        velY = 0;
        velX = 0;
        dom.classList.add('dragging');
        dom.setPointerCapture(e.pointerId);
      }

      function onPointerMove(e) {
        if (!dragging) return;
        const dx = e.clientX - lastPointerX;
        const dy = e.clientY - lastPointerY;
        lastPointerX = e.clientX;
        lastPointerY = e.clientY;

        const rotY = dx * dragSensitivity;
        const rotX = dy * dragSensitivity;

        sphere.rotation.y += rotY;
        sphere.rotation.x += rotX;

        velY = rotY;
        velX = rotX;
      }

      function onPointerUp(e) {
        if (!dragging) return;
        dragging = false;
        dom.classList.remove('dragging');
        try { dom.releasePointerCapture(e.pointerId); } catch (err) {}
      }

      dom.addEventListener('pointerdown', onPointerDown);
      dom.addEventListener('pointermove', onPointerMove);
      dom.addEventListener('pointerup', onPointerUp);
      dom.addEventListener('pointercancel', onPointerUp);

      const clock = new THREE.Clock();
      let angle = 0;

      function animate() {
        requestAnimationFrame(animate);

        const elapsed = clock.getElapsedTime();
        shaderMaterial.uniforms.uTime.value = elapsed;

        if (!dragging) {
          velY += (defaultVelY - velY) * easeBack;
          velX += (defaultVelX - velX) * easeBack;
          sphere.rotation.y += velY;
          sphere.rotation.x += velX;

          angle += 0.002;
          camera.position.x = Math.sin(angle) * 4;
          camera.position.z = Math.cos(angle) * 4;
          camera.position.y = 0.3 + Math.sin(angle * 0.5) * 0.2;
          camera.lookAt(0, 0, 0);
        }

        particles.rotation.y += 0.0002;
        particles.rotation.x += 0.00005;

        renderer.render(scene, camera);
      }
      animate();

      window.addEventListener('resize', () => {
        camera.aspect = window.innerWidth / window.innerHeight;
        camera.updateProjectionMatrix();
        renderer.setSize(window.innerWidth, window.innerHeight);
      });
    })();
  </script>
</body>
</html>`;
}
